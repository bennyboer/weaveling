use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{Act, Authorizer};
use axum::Extension;
use axum::Router;
use axum::extract::Query;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing;
use eventsourcing::Agent;

use crate::following::Asked;
use crate::{Broadcaster, socket};

#[async_trait]
pub trait Gate: Send + Sync {
    async fn admits(
        &self,
        agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;
}

pub struct ReadersOfTheProject(pub Arc<dyn Authorizer>);

#[async_trait]
impl Gate for ReadersOfTheProject {
    async fn admits(
        &self,
        agent: &Agent,
        project: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(self.0.may(agent, Act::Read, project).await?)
    }
}

pub async fn streaming_if_admitted(
    upgrade: WebSocketUpgrade,
    broadcaster: &Broadcaster,
    about: &str,
    agent: &Agent,
    gate: &dyn Gate,
) -> Response {
    match gate.admits(agent, about).await {
        Ok(true) => socket::streaming(upgrade, broadcaster.hear(about)),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub fn followed_if(
    path: &str,
    broadcaster: Broadcaster,
    about: fn(&Asked) -> Option<String>,
    gate: Arc<dyn Gate>,
) -> Router {
    Router::new().route(
        path,
        routing::get(
            move |upgrade: WebSocketUpgrade,
                  Extension(agent): Extension<Agent>,
                  Query(asked): Query<Asked>| async move {
                match about(&asked) {
                    Some(about) => {
                        streaming_if_admitted(upgrade, &broadcaster, &about, &agent, gate.as_ref())
                            .await
                    }
                    None => StatusCode::BAD_REQUEST.into_response(),
                }
            },
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum_test::{TestServer, TestWebSocket};

    use super::*;

    struct Only(&'static str);

    #[async_trait]
    impl Gate for Only {
        async fn admits(
            &self,
            _agent: &Agent,
            about: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            match about {
                "broken" => Err("the gate cannot tell".into()),
                about => Ok(about == self.0),
            }
        }
    }

    fn served(broadcaster: &Broadcaster) -> TestServer {
        TestServer::builder().http_transport().build(
            followed_if(
                "/live",
                broadcaster.clone(),
                |asked| asked.get("of").cloned(),
                Arc::new(Only("mine")),
            )
            .layer(Extension(Agent::Local)),
        )
    }

    async fn following(server: &TestServer, path: &str) -> TestWebSocket {
        server.get_websocket(path).await.into_websocket().await
    }

    #[tokio::test]
    async fn whoever_the_gate_admits_follows_what_they_asked_for() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster);
        let mut socket = following(&server, "/live?of=mine").await;

        broadcaster.tell("theirs", "not this".to_owned());
        broadcaster.tell("mine", "this".to_owned());

        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
                .await
                .expect("told while fresh"),
            "this"
        );
    }

    #[tokio::test]
    async fn whoever_the_gate_turns_away_finds_nothing_there() {
        let server = served(&Broadcaster::new());

        server
            .get_websocket("/live?of=theirs")
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_gate_that_cannot_tell_is_a_failure_and_asking_for_nothing_is_a_bad_request() {
        let server = served(&Broadcaster::new());

        server
            .get_websocket("/live?of=broken")
            .await
            .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
        server
            .get_websocket("/live")
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }
}
