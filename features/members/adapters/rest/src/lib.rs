use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::AccessWatch;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::{Agent, ServiceError};
use live::{About, Broadcaster, Gate, Refused, Sieve, Subscribed, Topic};
use live_contract::FrameDTO;
use members_contract::{
    ChangeRoleRequest, MemberDTO, MembersEventDTO, MembershipDTO, refused, role,
};
use members_core::{ListedMember, MembersCore, MembersCoreError, MembersError, Membership, Role};
use serde::Deserialize;
use serving::Problem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MembershipChanges;

pub fn router(members: MembersCore) -> Router {
    Router::new()
        .route("/members", routing::get(list))
        .route("/members/mine", routing::get(mine))
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

async fn mine(
    State(members): State<MembersCore>,
    Extension(agent): Extension<Agent>,
) -> Result<Json<Vec<MembershipDTO>>, Problem> {
    let held = members.memberships(&agent).await.map_err(problem_of)?;

    Ok(Json(held.iter().map(membership_dto).collect()))
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

fn to_dto(member: &ListedMember) -> MemberDTO {
    MemberDTO {
        account: member.account.to_string(),
        role: named(member.role).to_owned(),
        name: member.name.clone(),
        email: member.email.clone(),
    }
}

fn membership_dto(membership: &Membership) -> MembershipDTO {
    MembershipDTO {
        project: membership.project.to_string(),
        role: named(membership.role).to_owned(),
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

pub struct MembersTopic {
    members: MembersCore,
    broadcaster: Broadcaster<MembershipChanges>,
}

impl MembersTopic {
    pub fn new(members: MembersCore, broadcaster: Broadcaster<MembershipChanges>) -> Self {
        Self {
            members,
            broadcaster,
        }
    }

    async fn of_project(&self, agent: &Agent, project: &str) -> Result<Subscribed, Refused> {
        let admission =
            live::admitted(Arc::new(MembersGate(self.members.clone())), agent, project).await?;

        Ok(
            Subscribed::hearing(self.broadcaster.hear(MembershipChanges))
                .while_admitted(admission)
                .sifted(Arc::new(OfProject(project.to_owned()))),
        )
    }
}

struct MembersGate(MembersCore);

#[async_trait]
impl Gate for MembersGate {
    async fn admits(
        &self,
        agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(self.0.may_follow(agent, about).await?)
    }

    async fn watch(&self, about: &str) -> AccessWatch {
        self.0.watch_access(about)
    }
}

struct OfProject(String);

#[async_trait]
impl Sieve for OfProject {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(frame.of.as_deref() == Some(self.0.as_str()))
    }
}

#[async_trait]
impl Topic for MembersTopic {
    fn named(&self) -> &'static str {
        "members"
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        if let Some(project) = about.get("project") {
            return self.of_project(agent, project).await;
        }
        if about.get("mine").map(String::as_str) != Some("true") {
            return Err(Refused::NotThere);
        }

        Ok(
            Subscribed::hearing(self.broadcaster.hear(MembershipChanges)).sifted(Arc::new(
                NamingTheFollower {
                    agent: agent.clone(),
                    members: self.members.clone(),
                },
            )),
        )
    }
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
            MembersEventDTO::Disbanded => return Ok(false),
        };

        Ok(self.members.may_hear(&self.agent, &named))
    }
}

#[cfg(test)]
mod tests {
    use authorizing::AllowingEverything;
    use axum_test::TestServer;
    use clock::FixedClock;
    use eventsourcing::InMemoryEventStore;
    use live::testing::Subscriber;
    use members_catalog::InMemoryMembershipCatalog;
    use serde_json::{Value, json};
    use time::OffsetDateTime;

    use super::*;

    fn served(broadcaster: &Broadcaster<MembershipChanges>, agent: Agent) -> TestServer {
        let members = MembersCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryMembershipCatalog::new()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        TestServer::builder().http_transport().build(
            live::live_socket(live::Topics::of(vec![Arc::new(MembersTopic::new(
                members,
                broadcaster.clone(),
            ))]))
            .layer(Extension(agent)),
        )
    }

    fn ada() -> Agent {
        Agent::Account("account_ada".into())
    }

    async fn following(server: &TestServer, about: &[(&str, &str)]) -> Subscriber {
        let mut socket = Subscriber::on(server).await;
        socket.subscribe("mine", "members", about).await;

        socket
    }

    fn told(broadcaster: &Broadcaster<MembershipChanges>, project: &str, event: Value) {
        broadcaster.tell(
            MembershipChanges,
            json!({ "of": project, "version": 2, "at": "1970-01-01T00:00:00Z", "event": event })
                .to_string(),
        );
    }

    #[tokio::test]
    async fn a_follower_is_told_every_change_to_members_that_names_them_and_no_other() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster, ada());
        let mut socket = following(&server, &[("mine", "true")]).await;

        for event in [
            json!({ "name": "JOINED", "account": "account_grace", "role": "author" }),
            json!({ "name": "DISBANDED" }),
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
            let frame = socket.frame().await;
            assert_eq!(frame["of"], "project_1");
            names.push(frame["event"]["name"].clone());
        }
        assert_eq!(
            names,
            ["FOUNDED", "JOINED", "ROLE_CHANGED", "LEFT", "REMOVED"],
            "Ada is told what happened to her, nothing of Grace, and nothing of a disbanding, which the projects tell"
        );
    }

    #[tokio::test]
    async fn nobody_unsigned_is_told_anything() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster, Agent::Local);
        let mut socket = following(&server, &[("mine", "true")]).await;

        told(
            &broadcaster,
            "project_1",
            json!({ "name": "FOUNDED", "owner": "local" }),
        );

        socket.nothing().await;
    }

    #[tokio::test]
    async fn asking_for_neither_one_s_own_nor_a_project_s_members_finds_nothing_there() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster, ada());

        assert_eq!(
            following(&server, &[]).await.ended().await,
            "not-there",
            "a subscription says whose members it follows"
        );
    }

    #[tokio::test]
    async fn a_frame_that_tells_no_change_to_members_ends_the_subscription() {
        let broadcaster = Broadcaster::new();
        let server = served(&broadcaster, ada());
        let mut socket = following(&server, &[("mine", "true")]).await;

        told(&broadcaster, "project_1", json!({ "name": "RENAMED" }));
        told(
            &broadcaster,
            "project_1",
            json!({ "name": "FOUNDED", "owner": "account_ada" }),
        );

        assert_eq!(
            socket.ended().await,
            "failed",
            "only changes to members are told here, so anything else is a frame it cannot judge"
        );
    }
}
