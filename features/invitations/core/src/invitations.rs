use std::sync::Arc;

use acting::Acting;
use clock::Clock;
use eventsourcing::{Agent, EventStore, ServiceError};
use mailing::Mailer;
use throttling::Throttles;
use time::OffsetDateTime;

use crate::catalog::{InvitationCatalog, InvitationSummary, ListedInvitation};
use crate::error::InvitationsError;
use crate::id::InvitationId;
use crate::invitation::{DeclineReason, InvitationError, InvitationEvent};
use crate::service::InvitationService;

#[derive(Clone)]
pub struct InvitationsCore {
    service: InvitationService,
    catalog: Arc<dyn InvitationCatalog>,
    clock: Arc<dyn Clock>,
}

impl InvitationsCore {
    pub fn new(
        store: Arc<dyn EventStore<InvitationEvent>>,
        throttles: Arc<dyn Throttles>,
        catalog: Arc<dyn InvitationCatalog>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: InvitationService::new(store, throttles, mailer, clock.clone()),
            catalog,
            clock,
        }
    }

    pub async fn issue(
        &self,
        by: &Acting,
        to: &str,
        origin: &str,
    ) -> Result<InvitationSummary, InvitationsError> {
        operating(by)?;

        self.service.issue(to, origin, &by.agent).await
    }

    pub async fn withdraw(
        &self,
        by: &Acting,
        invitation: &InvitationId,
    ) -> Result<(), InvitationsError> {
        operating(by)?;

        self.service.withdraw(invitation, &by.agent).await
    }

    pub async fn list(
        &self,
        by: &Acting,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<ListedInvitation>, InvitationsError> {
        operating(by)?;
        let now = self.clock.now();

        Ok(self
            .catalog
            .listed_after(after, at_most)
            .await?
            .into_iter()
            .filter_map(|summary| {
                summary
                    .status(now)
                    .map(|status| ListedInvitation { summary, status })
            })
            .collect())
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
