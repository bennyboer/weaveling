use axum::Router;
use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::http::header::{
    CONTENT_SECURITY_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
};
use axum::middleware::{self, Next};
use axum::response::Response;

const HTTPS: &str = "https://";
const A_YEAR: &str = "max-age=31536000";
const NOSNIFF: &str = "nosniff";

/// What browsers are told to hold the instance's pages to.
#[derive(Debug, Clone)]
struct Shield {
    policy: HeaderValue,
    over_https: bool,
}

impl Shield {
    fn of(public_url: Option<&str>, changes_told_at: Option<u16>) -> Self {
        let changes = changes_told_at
            .map(|port| format!(" ws://*:{port} wss://*:{port}"))
            .unwrap_or_default();
        let policy = format!(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data:; font-src 'self'; connect-src 'self'{changes}; object-src 'none'; \
             base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
        );

        Self {
            policy: HeaderValue::from_str(&policy).expect("the policy is plain text"),
            over_https: public_url
                .is_some_and(|url| url.trim().to_ascii_lowercase().starts_with(HTTPS)),
        }
    }
}

/// Tells browsers to run only the instance's own scripts and reach only itself, never to show it inside another site's page,
/// never to guess what a file is, and, served over https, never to come back over http:
/// so a script that slipped in can do little, and no other site dresses the instance up to be clicked.
pub fn shielded(routes: Router, public_url: Option<&str>, changes_told_at: Option<u16>) -> Router {
    routes.layer(middleware::from_fn_with_state(
        Shield::of(public_url, changes_told_at),
        shield,
    ))
}

async fn shield(State(shield): State<Shield>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(CONTENT_SECURITY_POLICY, shield.policy);
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static(NOSNIFF));
    if shield.over_https {
        headers.insert(STRICT_TRANSPORT_SECURITY, HeaderValue::from_static(A_YEAR));
    }

    response
}

#[cfg(test)]
mod tests {
    use axum::routing;
    use axum_test::{TestResponse, TestServer};

    use super::*;

    async fn answered(public_url: Option<&str>, changes_told_at: Option<u16>) -> TestResponse {
        let routes = Router::new().route("/", routing::get(|| async { "a page" }));

        TestServer::new(shielded(routes, public_url, changes_told_at))
            .get("/")
            .await
    }

    fn policy_of(response: &TestResponse) -> String {
        response
            .header(CONTENT_SECURITY_POLICY)
            .to_str()
            .expect("a policy is plain text")
            .to_owned()
    }

    #[tokio::test]
    async fn a_page_runs_only_its_own_scripts_and_is_shown_in_no_other_site() {
        // given: an instance on this machine

        // when: a page is asked for
        let response = answered(None, None).await;

        // then: its own scripts and the client's wasm run, and no inline script
        let policy = policy_of(&response);
        assert!(
            policy.contains("script-src 'self' 'wasm-unsafe-eval';"),
            "{policy}"
        );
        // and: it reaches only itself
        assert!(policy.contains("connect-src 'self';"), "{policy}");
        // and: no other site frames it
        assert!(policy.contains("frame-ancestors 'none'"), "{policy}");
        // and: no file is taken for another kind than it is told
        assert_eq!(response.header(X_CONTENT_TYPE_OPTIONS), NOSNIFF);
    }

    #[tokio::test]
    async fn only_an_instance_served_over_https_holds_browsers_to_it() {
        // given: one instance served over https and one over http at localhost

        // when: each is asked for a page
        let secure = answered(Some("https://weaveling.example.com"), None).await;
        let plain = answered(Some("http://localhost:3000"), None).await;

        // then: only the first tells browsers never to come over http
        assert_eq!(secure.header(STRICT_TRANSPORT_SECURITY), A_YEAR);
        assert!(plain.maybe_header(STRICT_TRANSPORT_SECURITY).is_none());
    }

    #[tokio::test]
    async fn while_the_client_is_watched_its_page_may_hear_of_changes() {
        // given: an instance whose client cargo leptos watches, telling of changes at 3001

        // when: a page is asked for
        let response = answered(None, Some(3001)).await;

        // then: it may reach that port too
        let policy = policy_of(&response);
        assert!(
            policy.contains("connect-src 'self' ws://*:3001 wss://*:3001;"),
            "{policy}"
        );
    }
}
