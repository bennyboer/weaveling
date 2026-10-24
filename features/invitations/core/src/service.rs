use std::sync::Arc;

use acting::Acting;
use clock::Clock;
use email::{EmailAddress, InvalidEmailAddress};
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Version};
use mailing::{Mail, Mailer, Unsent};
use thiserror::Error;
use throttling::{Rule, Throttles, ThrottlesError};
use time::{Duration, OffsetDateTime};

use crate::catalog::{CatalogError, InvitationCatalog, InvitationSummary, ListedInvitation};
use crate::id::InvitationId;
use crate::invitation::{
    DeclineReason, Invitation, InvitationCommand, InvitationError, InvitationEvent, Outcome,
};
use crate::token::InvitationToken;

const INVITING: &str = "inviting";
const INVITING_RULE: Rule = Rule { free: 3 };

#[derive(Debug, Error)]
pub enum InvitationServiceError {
    #[error("only an operator may do this")]
    NotAnOperator,
    #[error(transparent)]
    InvalidEmail(#[from] InvalidEmailAddress),
    #[error("too many invitations to this address; wait {wait} before inviting it again")]
    Throttled { wait: Duration },
    #[error(transparent)]
    Events(#[from] ServiceError<InvitationError>),
    #[error(transparent)]
    Throttles(#[from] ThrottlesError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Unsent(#[from] Unsent),
}

#[derive(Clone)]
pub struct InvitationService {
    events: Arc<EventSourcingService<Invitation>>,
    throttles: Arc<dyn Throttles>,
    catalog: Arc<dyn InvitationCatalog>,
    mailer: Arc<dyn Mailer>,
    clock: Arc<dyn Clock>,
}

impl InvitationService {
    pub fn new(
        store: Arc<dyn EventStore<InvitationEvent>>,
        throttles: Arc<dyn Throttles>,
        catalog: Arc<dyn InvitationCatalog>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            throttles,
            catalog,
            mailer,
            clock,
        }
    }

    pub async fn issue(
        &self,
        by: &Acting,
        to: &str,
        origin: &str,
    ) -> Result<InvitationSummary, InvitationServiceError> {
        operating(by)?;
        let to = EmailAddress::new(to)?;
        self.not_throttled(&to).await?;
        self.throttles
            .count(INVITING, to.as_str(), self.clock.now())
            .await?;

        let now = self.clock.now();
        let id = InvitationId::generate(now);
        let until = now + Invitation::LASTS;
        let token = InvitationToken::drawn();
        let _landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                InvitationCommand::Issue {
                    to: to.clone(),
                    digest: token.digest(),
                    until,
                },
                &by.agent,
            )
            .await?;
        self.mailer
            .send(invitation_mail(&to, origin, &token))
            .await?;

        Ok(InvitationSummary {
            id,
            version: Version::of(1),
            to,
            issued_at: now,
            until,
            outcome: Outcome::Open,
            forgotten_after: until + InvitationSummary::EXPIRED_LISTED_FOR,
        })
    }

    pub async fn withdraw(
        &self,
        by: &Acting,
        invitation: &InvitationId,
    ) -> Result<(), InvitationServiceError> {
        operating(by)?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(invitation),
                InvitationCommand::Withdraw,
                &by.agent,
            )
            .await?;

        Ok(())
    }

    pub async fn list(
        &self,
        by: &Acting,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<ListedInvitation>, InvitationServiceError> {
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

    pub async fn accept(
        &self,
        invitation: &InvitationId,
        at: OffsetDateTime,
        by: &Agent,
    ) -> Result<(), InvitationServiceError> {
        let _landed = self
            .events
            .execute(
                &AggregateId::from(invitation),
                InvitationCommand::Accept { at },
                by,
            )
            .await?;

        Ok(())
    }

    pub async fn decline(
        &self,
        invitation: &InvitationId,
        reason: DeclineReason,
        by: &Agent,
    ) -> Result<(), InvitationServiceError> {
        let _landed = self
            .events
            .execute(
                &AggregateId::from(invitation),
                InvitationCommand::Decline { reason },
                by,
            )
            .await?;

        Ok(())
    }

    async fn not_throttled(&self, to: &EmailAddress) -> Result<(), InvitationServiceError> {
        let now = self.clock.now();
        let Some(tries) = self.throttles.tries(INVITING, to.as_str()).await? else {
            return Ok(());
        };
        if tries.is_forgotten(now) {
            return Ok(self.throttles.clear(INVITING, to.as_str()).await?);
        }

        match tries.wait_before_next(INVITING_RULE, now) {
            Some(wait) => Err(InvitationServiceError::Throttled { wait }),
            None => Ok(()),
        }
    }
}

fn operating(by: &Acting) -> Result<(), InvitationServiceError> {
    if by.operator {
        Ok(())
    } else {
        Err(InvitationServiceError::NotAnOperator)
    }
}

fn invitation_mail(to: &EmailAddress, origin: &str, token: &InvitationToken) -> Mail {
    let origin = origin.trim().trim_end_matches('/');

    Mail {
        to: to.to_string(),
        subject: "You are invited to Weaveling".to_owned(),
        body: format!(
            "You are invited to sign up to the Weaveling instance at {origin}. Follow this link within seven days to do so: {origin}/invitation#{}\n\nIf you did not expect this, you can ignore this mail.",
            token.as_str()
        ),
    }
}
