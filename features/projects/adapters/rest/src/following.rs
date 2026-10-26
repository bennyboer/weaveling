use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing;
use axum::{Extension, Router};
use eventsourcing::Agent;
use live::{Asked, Broadcaster, Gate, Sieve};
use live_contract::FrameDTO;
use projects_core::ProjectsCore;

pub const FOLLOWED: &str = "projects";

#[derive(Clone)]
struct Following {
    projects: ProjectsCore,
    broadcaster: Broadcaster,
}

pub fn live(projects: ProjectsCore, broadcaster: Broadcaster) -> Router {
    Router::new()
        .route("/projects/live", routing::get(follow))
        .with_state(Following {
            projects,
            broadcaster,
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
                projects: following.projects,
            }),
        );
    };

    live::streaming_if_admitted(
        upgrade,
        &following.broadcaster,
        project,
        &agent,
        &FollowersOf(following.projects),
    )
    .await
}

struct FollowersOf(ProjectsCore);

#[async_trait]
impl Gate for FollowersOf {
    async fn admits(
        &self,
        agent: &Agent,
        project: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(self.0.may_follow(project, agent).await?)
    }
}

struct ReadableBy {
    agent: Agent,
    projects: ProjectsCore,
}

#[async_trait]
impl Sieve for ReadableBy {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let Some(project) = frame.of.as_deref() else {
            return Ok(false);
        };

        Ok(self.projects.may_follow(project, &self.agent).await?)
    }
}
