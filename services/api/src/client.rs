use std::path::PathBuf;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing;
use tower_http::services::ServeDir;

const PAGE: &str = "page.html";
const LIVE_RELOAD: &str = include_str!("client/live_reload.js");

/// The built client this server hands out beside `/api`, and the port `cargo leptos watch` tells of changes on, while it watches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedClient {
    pub dir: PathBuf,
    pub reloading: Option<u16>,
}

/// Hands out the client's files, and its page for every other address, so the client's own routes open where they are linked to.
pub fn serving_the_client(routes: Router, client: ServedClient) -> Router {
    let page = routing::get(page).with_state(client.clone());

    routes
        .route("/", page.clone())
        .fallback_service(ServeDir::new(&client.dir).fallback(page))
}

async fn page(State(client): State<ServedClient>) -> Response {
    let Ok(written) = tokio::fs::read_to_string(client.dir.join(PAGE)).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "the client is not built yet: run cargo leptos build",
        )
            .into_response();
    };

    Html(match client.reloading {
        Some(port) => reloading(&written, port),
        None => written,
    })
    .into_response()
}

fn reloading(written: &str, port: u16) -> String {
    let script = format!(
        "<script>({})({port});</script>\n  </head>",
        LIVE_RELOAD.trim()
    );

    written.replacen("</head>", &script, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_listens_for_changes_on_the_port_told_and_only_once() {
        // given: the client's page
        let written = "<html><head><title>W</title></head><body></body></html>";

        // when: it is handed out while cargo leptos watches
        let handed = reloading(written, 3001);

        // then: the live reload is started with that port, before the head ends
        assert!(handed.contains("})(3001);</script>"));
        assert_eq!(handed.matches("<script>").count(), 1);
        assert!(handed.find("(3001)").unwrap() < handed.find("</head>").unwrap());
    }
}
