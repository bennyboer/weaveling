use std::collections::HashMap;
use std::future::Future;

use axum::Router;
use axum::extract::Query;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing;

use crate::Broadcaster;
use crate::socket;

pub type Asked = HashMap<String, String>;

pub fn followed_at(
    path: &str,
    broadcaster: Broadcaster,
    about: fn(&Asked) -> Option<String>,
) -> Router {
    Router::new().route(
        path,
        routing::get(
            move |upgrade: WebSocketUpgrade, Query(asked): Query<Asked>| async move {
                match about(&asked) {
                    Some(about) => socket::streaming(upgrade, broadcaster.hear(&about)),
                    None => StatusCode::BAD_REQUEST.into_response(),
                }
            },
        ),
    )
}

pub fn followed_from<Standing, Version>(
    path: &str,
    broadcaster: Broadcaster,
    about: fn(&Asked) -> Option<String>,
    standing: Standing,
) -> Router
where
    Standing: Fn(String) -> Version + Clone + Send + Sync + 'static,
    Version: Future<Output = Option<u64>> + Send + 'static,
{
    Router::new().route(
        path,
        routing::get(
            move |upgrade: WebSocketUpgrade, Query(asked): Query<Asked>| async move {
                let Some(about) = about(&asked) else {
                    return StatusCode::BAD_REQUEST.into_response();
                };
                let hearing = broadcaster.hear(&about);

                match standing(about).await {
                    Some(version) => socket::streaming_from(upgrade, version, hearing),
                    None => socket::streaming(upgrade, hearing),
                }
            },
        ),
    )
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum_test::TestServer;

    use super::*;

    fn the_project(asked: &Asked) -> Option<String> {
        asked.get("project").cloned()
    }

    fn served(router: Router) -> TestServer {
        TestServer::builder().http_transport().build(router)
    }

    #[tokio::test]
    async fn a_follower_hears_what_is_told_about_what_it_asked_for() {
        let broadcaster = Broadcaster::new();
        let server = served(followed_at("/live", broadcaster.clone(), the_project));
        let mut socket = server
            .get_websocket("/live?project=project_1")
            .await
            .into_websocket()
            .await;

        broadcaster.tell("project_2", "theirs".to_owned());
        broadcaster.tell("project_1", "mine".to_owned());

        assert_eq!(socket.receive_text().await, "mine");
    }

    #[tokio::test]
    async fn a_follower_who_does_not_say_what_it_follows_is_turned_away() {
        let server = served(followed_at("/live", Broadcaster::new(), the_project));

        server
            .get_websocket("/live")
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_follower_from_a_version_is_told_it_first() {
        let broadcaster = Broadcaster::new();
        let server = served(followed_from(
            "/live",
            broadcaster.clone(),
            the_project,
            |project: String| async move { (project == "project_1").then_some(7) },
        ));
        let mut known = server
            .get_websocket("/live?project=project_1")
            .await
            .into_websocket()
            .await;
        let mut unknown = server
            .get_websocket("/live?project=project_9")
            .await
            .into_websocket()
            .await;

        broadcaster.tell("project_1", "mine".to_owned());
        broadcaster.tell("project_9", "theirs".to_owned());

        assert_eq!(known.receive_text().await, r#"{"version":7}"#);
        assert_eq!(known.receive_text().await, "mine");
        assert_eq!(
            unknown.receive_text().await,
            "theirs",
            "what stands nowhere has no version to say first"
        );
    }
}
