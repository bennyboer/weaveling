use std::net::IpAddr;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::header::{HOST, ORIGIN};
use axum::http::uri::Authority;
use axum::http::{HeaderMap, HeaderName, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

const LOCALHOST: &str = "localhost";

/// Where a page comes from, as browsers report it: its scheme, and its host with the port.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    scheme: String,
    authority: String,
}

impl Origin {
    fn parsed(url: &str) -> Option<Self> {
        let uri = url.parse::<Uri>().ok()?;

        Some(Self {
            scheme: uri.scheme_str()?.to_ascii_lowercase(),
            authority: uri.authority()?.as_str().to_ascii_lowercase(),
        })
    }
}

/// Where the instance is opened, as configured.
/// Its own pages come from there, and requests are made to its name.
#[derive(Debug, Clone)]
struct Served {
    public: Option<Origin>,
    hosts: Vec<String>,
}

impl Served {
    fn at(public_url: Option<&str>, hosts: Vec<String>) -> Self {
        Self {
            public: public_url.and_then(Origin::parsed),
            hosts,
        }
    }

    fn names(&self, host: &str) -> bool {
        let Some(name) = name_of(host) else {
            return false;
        };

        name.trim_matches(['[', ']']).parse::<IpAddr>().is_ok()
            || name == LOCALHOST
            || self.hosts.contains(&name)
            || self
                .public
                .as_ref()
                .and_then(|public| name_of(&public.authority))
                .is_some_and(|public| public == name)
    }

    fn made_by_its_own_page(&self, origin: &str, host: &str) -> bool {
        let Some(page) = Origin::parsed(origin) else {
            return false;
        };

        page.authority == host.to_ascii_lowercase() || self.public.as_ref() == Some(&page)
    }
}

/// Answers only requests made to a name this instance is served at.
/// When a page made the request, it must be one of the instance's own pages.
/// This keeps other sites from reaching the sockets, which browsers do not restrict to their own origin.
/// It also keeps other sites from reaching the API by DNS rebinding.
pub fn answering_only_its_own(
    routes: Router,
    public_url: Option<&str>,
    hosts: Vec<String>,
) -> Router {
    routes.layer(middleware::from_fn_with_state(
        Served::at(public_url, hosts),
        admit,
    ))
}

async fn admit(State(served): State<Served>, request: Request, next: Next) -> Response {
    let Some(host) = host_of(&request) else {
        return StatusCode::MISDIRECTED_REQUEST.into_response();
    };
    if !served.names(&host) {
        tracing::warn!(
            "refused a request made to {host}, which is not where this instance is served; if it is, set public_url to it"
        );
        return StatusCode::MISDIRECTED_REQUEST.into_response();
    }
    if let Some(origin) = header(request.headers(), ORIGIN)
        && !served.made_by_its_own_page(origin, &host)
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    next.run(request).await
}

fn host_of(request: &Request) -> Option<String> {
    header(request.headers(), HOST)
        .map(str::to_owned)
        .or_else(|| {
            request
                .uri()
                .authority()
                .map(|authority| authority.to_string())
        })
}

fn header(headers: &HeaderMap, name: HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn name_of(host: &str) -> Option<String> {
    let authority = host.parse::<Authority>().ok()?;

    Some(authority.host().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use axum::routing;
    use axum_test::TestServer;

    use super::*;

    const PUBLIC_URL: &str = "https://weaveling.example.com";

    fn a_server(public_url: Option<&str>) -> TestServer {
        a_server_also_at(public_url, &[])
    }

    fn a_server_also_at(public_url: Option<&str>, hosts: &[&str]) -> TestServer {
        let routes = Router::new().route("/api/service/health", routing::get(|| async { "ok" }));

        TestServer::new(answering_only_its_own(
            routes,
            public_url,
            hosts.iter().map(|host| (*host).to_owned()).collect(),
        ))
    }

    async fn asked(server: &TestServer, host: &str, origin: Option<&str>) -> StatusCode {
        let mut request = server.get("/api/service/health").add_header(HOST, host);
        if let Some(origin) = origin {
            request = request.add_header(ORIGIN, origin);
        }

        request.await.status_code()
    }

    #[tokio::test]
    async fn a_page_of_another_site_is_refused() {
        // given: an instance on this machine, with no public URL
        let server = a_server(None);

        // when: a page of another site asks it, as any page may open a socket to it
        let status = asked(&server, "localhost:3000", Some("https://elsewhere.example")).await;

        // then: it is refused
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_name_not_its_own_is_refused_even_from_a_page_of_that_name() {
        // given: an instance served at its public URL
        let server = a_server(Some(PUBLIC_URL));

        // when: it is asked by a name another site made point to it, as DNS rebinding does
        let status = asked(
            &server,
            "rebound.example:3000",
            Some("http://rebound.example:3000"),
        )
        .await;

        // then: it is refused, the page and the name agreeing being no proof of anything
        assert_eq!(status, StatusCode::MISDIRECTED_REQUEST);
    }

    #[tokio::test]
    async fn its_own_pages_are_answered_wherever_it_is_opened() {
        // given: an instance served at its public URL, behind a proxy
        let server = a_server(Some(PUBLIC_URL));

        // when: its pages ask it by the public URL, through a proxy naming where it listens,
        // at localhost, and at an address
        let asking = [
            ("weaveling.example.com", PUBLIC_URL),
            ("127.0.0.1:3000", PUBLIC_URL),
            ("localhost:3000", "http://localhost:3000"),
            ("192.0.2.7:3000", "http://192.0.2.7:3000"),
            ("[::1]:3000", "http://[::1]:3000"),
        ];
        let mut statuses = Vec::new();
        for (host, origin) in asking {
            statuses.push((host, asked(&server, host, Some(origin)).await));
        }

        // then: each is answered
        assert!(
            statuses.iter().all(|(_, status)| *status == StatusCode::OK),
            "{statuses:?}"
        );
    }

    #[tokio::test]
    async fn a_request_no_page_made_is_answered() {
        // given: an instance on this machine
        let server = a_server(None);

        // when: it is asked with no origin, as a link followed or a program asks
        let status = asked(&server, "127.0.0.1:3000", None).await;

        // then: it is answered, no other site having made it
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_page_of_no_origin_is_refused() {
        // given: an instance on this machine
        let server = a_server(None);

        // when: a sandboxed page or a file asks it, its origin told as null
        let status = asked(&server, "localhost:3000", Some("null")).await;

        // then: it is refused
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_name_it_is_told_of_is_answered_and_its_pages_too() {
        // given: an instance told it is also opened as mybox.local
        let server = a_server_also_at(None, &["mybox.local"]);

        // when: a page of that name asks it
        let status = asked(&server, "MyBox.local:3000", Some("http://mybox.local:3000")).await;

        // then: it is answered
        assert_eq!(status, StatusCode::OK);
    }
}
