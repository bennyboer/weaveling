use std::sync::Arc;

use clock::Clock;
use eventsourcing::{AccountId, Agent, EventStore};

use crate::catalog::{MembershipCatalog, ProjectMembers};
use crate::error::MembersCoreError;
use crate::members::{MembersEvent, ProjectLink};
use crate::service::MembersService;

#[derive(Clone)]
pub struct MembersCore {
    service: MembersService,
    catalog: Arc<dyn MembershipCatalog>,
}

impl MembersCore {
    pub fn new(
        store: Arc<dyn EventStore<MembersEvent>>,
        catalog: Arc<dyn MembershipCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: MembersService::new(store, clock),
            catalog,
        }
    }

    pub async fn found(&self, project: &str, owner: &str) -> Result<(), MembersCoreError> {
        self.service
            .found(
                &ProjectLink::from(project),
                AccountId::from(owner),
                &Agent::System,
            )
            .await
    }

    pub async fn catalogue(&self, project: &ProjectLink) -> Result<(), MembersCoreError> {
        let standing = self.service.latest(project).await?;

        Ok(self
            .catalog
            .remember(&ProjectMembers::of(
                project.clone(),
                standing.version,
                &standing.state,
            ))
            .await?)
    }

    pub fn may_hear(&self, agent: &Agent, about: &str) -> bool {
        matches!(agent, Agent::Account(follower) if follower.as_str() == about)
    }
}
