use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{Act, Authorizer};
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing;
use axum::{Extension, Router};
use eventsourcing::Agent;
use live::{Asked, Broadcaster, Sieve};
use serde_json::Value;

pub const FOLLOWED: &str = "projects";

#[derive(Clone)]
struct Following {
    broadcaster: Broadcaster,
    authorizer: Arc<dyn Authorizer>,
}

pub fn live(broadcaster: Broadcaster, authorizer: Arc<dyn Authorizer>) -> Router {
    Router::new()
        .route("/projects/live", routing::get(follow))
        .with_state(Following {
            broadcaster,
            authorizer,
        })
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(following): State<Following>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<Asked>,
) -> Response {
    let Some(project) = asked.get("project") else {
        return live::streaming_sifted(
            upgrade,
            following.broadcaster.hear(FOLLOWED),
            Arc::new(ReadableBy {
                agent,
                authorizer: following.authorizer,
            }),
        );
    };

    match following.authorizer.may(&agent, Act::Read, project).await {
        Ok(true) => live::streaming(upgrade, following.broadcaster.hear(project)),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

struct ReadableBy {
    agent: Agent,
    authorizer: Arc<dyn Authorizer>,
}

#[async_trait]
impl Sieve for ReadableBy {
    async fn lets_through(&self, frame: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let frame: Value = serde_json::from_str(frame)?;
        let Some(project) = frame["of"].as_str() else {
            return Ok(false);
        };

        Ok(self.authorizer.may(&self.agent, Act::Read, project).await?)
    }
}
