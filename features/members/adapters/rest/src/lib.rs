use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::{Agent, ServiceError};
use live::{Broadcaster, Sieve};
use live_contract::FrameDTO;
use members_contract::{ChangeRoleRequest, MemberDTO, MembersEventDTO, refused, role};
use members_core::{Member, MembersCore, MembersCoreError, MembersError, Role};
use serde::Deserialize;
use serving::Problem;

pub const FOLLOWED: &str = "memberships";

pub fn router(members: MembersCore) -> Router {
    Router::new()
        .route("/members", routing::get(list))
        .route(
            "/members/{project}/accounts/{account}",
            routing::patch(change_role).delete(remove),
        )
        .with_state(members)
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn list(
    State(members): State<MembersCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<MemberDTO>>, Problem> {
    let listed = members
        .members(&asked.project, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(listed.iter().map(to_dto).collect()))
}

async fn change_role(
    State(members): State<MembersCore>,
    Extension(agent): Extension<Agent>,
    Path((project, account)): Path<(String, String)>,
    Json(request): Json<ChangeRoleRequest>,
) -> Result<Json<Vec<MemberDTO>>, Problem> {
    let to = to_role(&request.role).ok_or_else(|| {
        Problem::unprocessable(
            refused::ROLE_UNKNOWN,
            format!("{} is no role in a project", request.role),
        )
    })?;
    let changed = members
        .change_role(&project, &account, to, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(changed.iter().map(to_dto).collect()))
}

async fn remove(
    State(members): State<MembersCore>,
    Extension(agent): Extension<Agent>,
    Path((project, account)): Path<(String, String)>,
) -> Result<StatusCode, Problem> {
    let leaving = matches!(&agent, Agent::Account(acting) if acting.as_str() == account);
    if leaving {
        members.leave(&project, &agent).await.map_err(problem_of)?;
    } else {
        members
            .remove(&project, &account, &agent)
            .await
            .map_err(problem_of)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

fn to_dto(member: &Member) -> MemberDTO {
    MemberDTO {
        account: member.account.to_string(),
        role: named(member.role).to_owned(),
    }
}

fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}

fn to_role(named: &str) -> Option<Role> {
    match named {
        role::OWNER => Some(Role::Owner),
        role::AUTHOR => Some(Role::Author),
        role::READER => Some(Role::Reader),
        _ => None,
    }
}

fn problem_of(refusal: MembersCoreError) -> Problem {
    match refusal {
        MembersCoreError::ProjectNotThere => {
            Problem::not_found(serving::NOT_FOUND, MembersCoreError::ProjectNotThere)
        }
        MembersCoreError::NotAnOwner => {
            Problem::forbidden(refused::NOT_AN_OWNER, MembersCoreError::NotAnOwner)
        }
        MembersCoreError::Events(ServiceError::Refused(MembersError::LastOwner)) => {
            Problem::refused(refused::LAST_OWNER, MembersError::LastOwner)
        }
        MembersCoreError::Events(ServiceError::Refused(MembersError::NotMember)) => {
            Problem::not_found(refused::NOT_A_MEMBER, MembersError::NotMember)
        }
        MembersCoreError::Events(ServiceError::NotFound { .. }) => {
            Problem::not_found(serving::NOT_FOUND, "there is no such project")
        }
        beyond_reach => Problem::unexpected(beyond_reach),
    }
}

#[derive(Clone)]
struct Following {
    members: MembersCore,
    broadcaster: Broadcaster,
}

pub fn live(members: MembersCore, broadcaster: Broadcaster) -> Router {
    Router::new()
        .route("/members/live", routing::get(follow))
        .with_state(Following {
            members,
            broadcaster,
        })
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(following): State<Following>,
    Extension(agent): Extension<Agent>,
) -> Response {
    live::streaming_sifted(
        upgrade,
        following.broadcaster.hear(FOLLOWED),
        Arc::new(NamingTheFollower {
            agent,
            members: following.members,
        }),
    )
}

struct NamingTheFollower {
    agent: Agent,
    members: MembersCore,
}

#[async_trait]
impl Sieve for NamingTheFollower {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let named = match serde_json::from_value::<MembersEventDTO>(frame.event.clone())? {
            MembersEventDTO::Founded { owner } => owner,
            MembersEventDTO::Joined { account, .. }
            | MembersEventDTO::RoleChanged { account, .. }
            | MembersEventDTO::Left { account }
            | MembersEventDTO::Removed { account } => account,
        };

        Ok(self.members.may_hear(&self.agent, &named))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use authorizing::AllowingEverything;
    use axum_test::{TestServer, TestWebSocket};
    use clock::FixedClock;
    use eventsourcing::InMemoryEventStore;
    use members_catalog::InMemoryMembershipCatalog;
    use serde_json::{Value, json};
    use time::OffsetDateTime;

    use super::*;

    fn served(broadcaster: &Broadcaster, agent: Agent) -> TestServer {
        let members = MembersCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryMembershipCatalog::new()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        TestServer::builder()
            .http_transport()
            .build(live(members, broadcaster.clone()).layer(Extension(agent)))
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
