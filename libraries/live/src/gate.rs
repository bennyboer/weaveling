use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::AccessWatch;
use axum::Extension;
use axum::Router;
use axum::extract::Query;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing;
use eventsourcing::Agent;

use crate::following::Asked;
use crate::socket::Admission;
use crate::{Broadcaster, socket};

#[async_trait]
pub trait Gate: Send + Sync {
    async fn admits(
        &self,
        agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    fn watch(&self, _about: &str) -> AccessWatch {
        AccessWatch::still()
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

async fn streaming_while_admitted(
    upgrade: WebSocketUpgrade,
    broadcaster: &Broadcaster,
    about: String,
    agent: Agent,
    gate: Arc<dyn Gate>,
) -> Response {
    let watch = gate.watch(&about);

    match gate.admits(&agent, &about).await {
        Ok(true) => socket::streaming_while(
            upgrade,
            broadcaster.hear(&about),
            Admission {
                watch,
                gate,
                agent,
                about,
            },
        ),
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
                        streaming_while_admitted(upgrade, &broadcaster, about, agent, gate.clone())
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

    use std::sync::Mutex;

    use authorizing::AccessChanges;
    use axum_test::{TestServer, TestWebSocket, WsMessage};
    use live_contract::{ACCESS_CHANGED, NO_LONGER_ADMITTED, NoticeDTO};

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

    struct Watched {
        admitting: Mutex<Result<bool, ()>>,
        changes: AccessChanges,
    }

    #[async_trait]
    impl Gate for Watched {
        async fn admits(
            &self,
            _agent: &Agent,
            _about: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            (*self.admitting.lock().expect("the gate is not poisoned"))
                .map_err(|()| "the gate cannot tell".into())
        }

        fn watch(&self, about: &str) -> AccessWatch {
            self.changes.watch(about)
        }
    }

    async fn watched_and_following() -> (Arc<Watched>, Broadcaster, TestWebSocket, TestServer) {
        let gate = Arc::new(Watched {
            admitting: Mutex::new(Ok(true)),
            changes: AccessChanges::new(),
        });
        let broadcaster = Broadcaster::new();
        let server = TestServer::builder().http_transport().build(
            followed_if(
                "/live",
                broadcaster.clone(),
                |asked| asked.get("of").cloned(),
                gate.clone(),
            )
            .layer(Extension(Agent::Local)),
        );
        let socket = following(&server, "/live?of=mine").await;

        (gate, broadcaster, socket, server)
    }

    async fn closed_as_no_longer_admitted(socket: &mut TestWebSocket) -> bool {
        matches!(
            tokio::time::timeout(Duration::from_secs(1), socket.receive_message()).await,
            Ok(WsMessage::Close(Some(frame))) if u16::from(frame.code) == NO_LONGER_ADMITTED
        )
    }

    #[tokio::test]
    async fn a_follower_no_longer_admitted_after_a_change_is_closed_out() {
        for after in [Ok(false), Err(())] {
            let (gate, _broadcaster, mut socket, _server) = watched_and_following().await;
            *gate.admitting.lock().expect("the gate is not poisoned") = after;

            gate.changes.tell("mine");

            assert!(
                closed_as_no_longer_admitted(&mut socket).await,
                "neither turned away nor unanswered may keep following"
            );
        }
    }

    #[tokio::test]
    async fn a_follower_still_admitted_is_told_its_access_changed_and_follows_on() {
        let (gate, broadcaster, mut socket, _server) = watched_and_following().await;
        gate.changes.tell("mine");
        let told = tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
            .await
            .expect("told of the change");
        assert_eq!(
            serde_json::from_str::<NoticeDTO>(&told).expect("a notice"),
            NoticeDTO {
                notice: ACCESS_CHANGED.to_owned()
            },
            "its role may be another now, so its client asks again"
        );
        *gate.admitting.lock().expect("the gate is not poisoned") = Ok(false);
        gate.changes.tell("theirs");
        tokio::time::sleep(Duration::from_millis(50)).await;

        broadcaster.tell("mine", "still told".to_owned());

        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
                .await
                .expect("told while admitted"),
            "still told"
        );
    }
}
