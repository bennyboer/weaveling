use std::sync::Arc;

use acting::Acting;
use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventStore, ServiceError};
use mailing::Mailer;
use throttling::Throttles;
use time::OffsetDateTime;

use crate::catalog::{InvitationCatalog, InvitationSummary, ListedInvitation, Person, ProjectName};
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
        if self.catalog.has_project_tombstone(&project).await? {
            return Err(InvitationsError::ProjectNotThere);
        }

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

    pub async fn remember_project_name(&self, named: &ProjectName) -> Result<(), InvitationsError> {
        Ok(self.catalog.remember_project_name(named).await?)
    }

    pub async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), InvitationsError> {
        Ok(self.catalog.lay_project_tombstone(project, at).await?)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, InvitationsError> {
        Ok(self
            .catalog
            .forget_project_tombstones_laid_before(before, at_most)
            .await?)
    }

    pub async fn remember_person(&self, person: &Person) -> Result<(), InvitationsError> {
        Ok(self.catalog.remember_person(person).await?)
    }

    pub async fn catalogue(&self, invitation: &InvitationId) -> Result<(), InvitationsError> {
        let standing = self.service.latest(invitation).await?;
        if standing.state.is_retired() {
            return Ok(self.catalog.forget(invitation).await?);
        }

        let summary = InvitationSummary::of(*invitation, standing.version, &standing.state);
        self.catalog.remember(&summary).await?;

        match &summary.scope {
            Scope::Project { project, .. }
                if self.catalog.has_project_tombstone(project).await? =>
            {
                settled(self.service.withdraw(invitation, &Agent::System).await)
            }
            _ => Ok(()),
        }
    }

    pub async fn sweep(&self, project: &str) -> Result<(), InvitationsError> {
        let project = ProjectLink::from(project);
        let mut after = None;

        loop {
            let page = self
                .catalog
                .in_project_after(&project, after.as_ref(), SWEPT_EACH_PAGE)
                .await?;
            for summary in &page {
                settled(self.service.withdraw(&summary.id, &Agent::System).await)?;
            }
            match page.last() {
                Some(last) if page.len() == SWEPT_EACH_PAGE => after = Some(last.id),
                _ => return Ok(()),
            }
        }
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

const SWEPT_EACH_PAGE: usize = 100;

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
