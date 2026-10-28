use std::sync::Arc;

use acting::Acting;
use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventStore, ServiceError};
use mailing::Mailer;
use throttling::Throttles;
use time::OffsetDateTime;

use crate::catalog::{InvitationCatalog, InvitationSummary, ListedInvitation, Person};
use crate::error::InvitationsError;
use crate::id::InvitationId;
use crate::invitation::{DeclineReason, InvitationError, InvitationEvent, KIND};
use crate::scope::{ProjectLink, Role, Scope};
use crate::service::InvitationService;

#[derive(Clone)]
pub struct InvitationsCore {
    service: InvitationService,
    catalog: Arc<dyn InvitationCatalog>,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl InvitationsCore {
    pub fn new(
        store: Arc<dyn EventStore<InvitationEvent>>,
        throttles: Arc<dyn Throttles>,
        catalog: Arc<dyn InvitationCatalog>,
        mailer: Arc<dyn Mailer>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: InvitationService::new(store, throttles, mailer, clock.clone()),
            catalog,
            authorizer,
            clock,
        }
    }

    pub async fn invite_to_instance(
        &self,
        by: &Acting,
        to: &str,
        origin: &str,
    ) -> Result<InvitationSummary, InvitationsError> {
        operating(by)?;

        self.service
            .issue(to, origin, Scope::Instance, true, &by.agent)
            .await
    }

    pub async fn invite_to_project(
        &self,
        by: &Acting,
        project: &str,
        role: Role,
        to: &str,
        origin: &str,
    ) -> Result<InvitationSummary, InvitationsError> {
        let project = ProjectLink::from(project);
        self.owning(by, &project).await?;

        self.service
            .issue(
                to,
                origin,
                Scope::Project { project, role },
                by.operator,
                &by.agent,
            )
            .await
    }

    pub async fn withdraw(
        &self,
        by: &Acting,
        invitation: &InvitationId,
    ) -> Result<(), InvitationsError> {
        match self.service.latest(invitation).await?.state.scope() {
            Scope::Instance => operating(by)?,
            Scope::Project { project, .. } => {
                self.owning(by, project)
                    .await
                    .map_err(|refused| match refused {
                        InvitationsError::ProjectNotThere => not_found(invitation),
                        refused => refused,
                    })?
            }
        }

        self.service.withdraw(invitation, &by.agent).await
    }

    pub async fn instance_invitations(
        &self,
        by: &Acting,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<ListedInvitation>, InvitationsError> {
        operating(by)?;

        Ok(self.listed(self.catalog.listed_after(after, at_most).await?))
    }

    pub async fn project_invitations(
        &self,
        by: &Acting,
        project: &str,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<ListedInvitation>, InvitationsError> {
        let project = ProjectLink::from(project);
        self.owning(by, &project).await?;

        Ok(self.listed(
            self.catalog
                .in_project_after(&project, after, at_most)
                .await?,
        ))
    }

    fn listed(&self, summaries: Vec<InvitationSummary>) -> Vec<ListedInvitation> {
        let now = self.clock.now();

        summaries
            .into_iter()
            .filter_map(|summary| {
                summary
                    .status(now)
                    .map(|status| ListedInvitation { summary, status })
            })
            .collect()
    }

    async fn owning(&self, by: &Acting, project: &ProjectLink) -> Result<(), InvitationsError> {
        if self
            .authorizer
            .may(&by.agent, Act::Manage, project.as_str())
            .await?
        {
            Ok(())
        } else if self
            .authorizer
            .may(&by.agent, Act::Read, project.as_str())
            .await?
        {
            Err(InvitationsError::NotAnOwner)
        } else {
            Err(InvitationsError::ProjectNotThere)
        }
    }

    pub async fn taken_up(
        &self,
        invitation: &InvitationId,
        at: OffsetDateTime,
        by: &Agent,
    ) -> Result<(), InvitationsError> {
        settled(self.service.accept(invitation, at, by).await)
    }

    pub async fn declined(
        &self,
        invitation: &InvitationId,
        reason: DeclineReason,
    ) -> Result<(), InvitationsError> {
        settled(
            self.service
                .decline(invitation, reason, &Agent::System)
                .await,
        )
    }

    pub async fn remember_person(&self, person: &Person) -> Result<(), InvitationsError> {
        Ok(self.catalog.remember_person(person).await?)
    }

    pub async fn catalogue(&self, invitation: &InvitationId) -> Result<(), InvitationsError> {
        let standing = self.service.latest(invitation).await?;
        if standing.state.is_retired() {
            return Ok(self.catalog.forget(invitation).await?);
        }

        Ok(self
            .catalog
            .remember(&InvitationSummary::of(
                *invitation,
                standing.version,
                &standing.state,
            ))
            .await?)
    }

    pub async fn retire_due(&self, at_most: usize) -> Result<usize, InvitationsError> {
        let due = self
            .catalog
            .due_for_retiring(self.clock.now(), at_most)
            .await?;
        for invitation in &due {
            self.service.retire(invitation, &Agent::System).await?;
        }

        Ok(due.len())
    }
}

fn not_found(invitation: &InvitationId) -> InvitationsError {
    InvitationsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(invitation),
        kind: KIND,
    })
}

fn operating(by: &Acting) -> Result<(), InvitationsError> {
    if by.operator {
        Ok(())
    } else {
        Err(InvitationsError::NotAnOperator)
    }
}

fn settled(outcome: Result<(), InvitationsError>) -> Result<(), InvitationsError> {
    match outcome {
        Err(InvitationsError::Events(ServiceError::Refused(
            InvitationError::AlreadyAccepted | InvitationError::Withdrawn,
        ))) => Ok(()),
        outcome => outcome,
    }
}
