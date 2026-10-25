use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::response::Response;
use axum::routing;
use axum::{Extension, Router};
use eventsourcing::Agent;
use live::{Broadcaster, Sieve};
use live_contract::FrameDTO;
use members_contract::MembersEventDTO;

pub const FOLLOWED: &str = "memberships";

pub fn live(broadcaster: Broadcaster) -> Router {
    Router::new()
        .route("/members/live", routing::get(follow))
        .with_state(broadcaster)
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(broadcaster): State<Broadcaster>,
    Extension(agent): Extension<Agent>,
) -> Response {
    live::streaming_sifted(
        upgrade,
        broadcaster.hear(FOLLOWED),
        Arc::new(NamingTheFollower(agent)),
    )
}

struct NamingTheFollower(Agent);

#[async_trait]
impl Sieve for NamingTheFollower {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let Agent::Account(follower) = &self.0 else {
            return Ok(false);
        };
        let named = match serde_json::from_value::<MembersEventDTO>(frame.event.clone())? {
            MembersEventDTO::Founded { owner } => owner,
            MembersEventDTO::Joined { account, .. }
            | MembersEventDTO::RoleChanged { account, .. }
            | MembersEventDTO::Left { account }
            | MembersEventDTO::Removed { account } => account,
        };

        Ok(named == follower.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum_test::{TestServer, TestWebSocket};
    use serde_json::{Value, json};

    use super::*;

    fn served(broadcaster: &Broadcaster, agent: Agent) -> TestServer {
        TestServer::builder()
            .http_transport()
            .build(live(broadcaster.clone()).layer(Extension(agent)))
    }

    fn ada() -> Agent {
        Agent::Account("account_ada".into())
    }

    async fn following(server: &TestServer) -> TestWebSocket {
        server
            .get_websocket("/members/live")
            .await
            .into_websocket()
            .await
    }

    fn told(broadcaster: &Broadcaster, project: &str, event: Value) {
        broadcaster.tell(
            FOLLOWED,
            json!({ "of": project, "version": 2, "at": "1970-01-01T00:00:00Z", "event": event })
                .to_string(),
        );
    }

    async fn heard(socket: &mut TestWebSocket) -> Value {
        serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
                .await
                .expect("a change is told while it is fresh"),
        )
        .expect("a frame is JSON")
    }

    #[tokio::test]
    async fn a_follower_is_told_every_change_to_members_that_names_them_and_no_other() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster, ada());
        let mut socket = following(&server).await;

        for event in [
            json!({ "name": "JOINED", "account": "account_grace", "role": "author" }),
            json!({ "name": "FOUNDED", "owner": "account_ada" }),
            json!({ "name": "REMOVED", "account": "account_grace" }),
            json!({ "name": "JOINED", "account": "account_ada", "role": "reader" }),
            json!({ "name": "ROLE_CHANGED", "account": "account_ada", "to": "author" }),
            json!({ "name": "LEFT", "account": "account_ada" }),
            json!({ "name": "FOUNDED", "owner": "account_grace" }),
            json!({ "name": "REMOVED", "account": "account_ada" }),
        ] {
            told(&broadcaster, "project_1", event);
        }

        let mut names = Vec::new();
        for _ in 0..5 {
            let frame = heard(&mut socket).await;
            assert_eq!(frame["of"], "project_1");
            names.push(frame["event"]["name"].clone());
        }
        assert_eq!(
            names,
            ["FOUNDED", "JOINED", "ROLE_CHANGED", "LEFT", "REMOVED"],
            "Ada is told what happened to her, and nothing of Grace"
        );
    }

    #[tokio::test]
    async fn nobody_unsigned_is_told_anything() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster, Agent::Local);
        let mut socket = following(&server).await;

        told(
            &broadcaster,
            "project_1",
            json!({ "name": "FOUNDED", "owner": "local" }),
        );

        assert!(
            tokio::time::timeout(Duration::from_millis(200), socket.receive_text())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_frame_that_tells_no_change_to_members_ends_the_socket() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster, ada());
        let mut socket = following(&server).await;

        told(&broadcaster, "project_1", json!({ "name": "RENAMED" }));
        told(
            &broadcaster,
            "project_1",
            json!({ "name": "FOUNDED", "owner": "account_ada" }),
        );

        assert!(
            tokio::spawn(async move { socket.receive_text().await })
                .await
                .is_err(),
            "only changes to members are told here, so anything else is a frame it cannot judge"
        );
    }
}
