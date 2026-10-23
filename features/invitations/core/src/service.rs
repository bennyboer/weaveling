use std::sync::Arc;

use acting::Acting;
use clock::Clock;
use email::{EmailAddress, InvalidEmailAddress};
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, ServiceError};
use mailing::{Mail, Mailer, Unsent};
use thiserror::Error;
use throttling::{Rule, Throttles, ThrottlesError};
use time::{Duration, OffsetDateTime};

use crate::id::InvitationId;
use crate::invitation::{Invitation, InvitationCommand, InvitationError, InvitationEvent};
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
    Unsent(#[from] Unsent),
}

#[derive(Clone)]
pub struct InvitationService {
    events: Arc<EventSourcingService<Invitation>>,
    throttles: Arc<dyn Throttles>,
    mailer: Arc<dyn Mailer>,
    clock: Arc<dyn Clock>,
}

impl InvitationService {
    pub fn new(
        store: Arc<dyn EventStore<InvitationEvent>>,
        throttles: Arc<dyn Throttles>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            throttles,
            mailer,
            clock,
        }
    }

    pub async fn issue(
        &self,
        by: &Acting,
        to: &str,
        origin: &str,
    ) -> Result<InvitationId, InvitationServiceError> {
        operating(by)?;
        let to = EmailAddress::new(to)?;
        self.not_throttled(&to).await?;
        self.throttles
            .count(INVITING, to.as_str(), self.clock.now())
            .await?;

        let id = InvitationId::generate(self.clock.now());
        let token = InvitationToken::drawn();
        let _landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                InvitationCommand::Issue {
                    to: to.clone(),
                    digest: token.digest(),
                    until: self.clock.now() + Invitation::LASTS,
                },
                &by.agent,
            )
            .await?;
        self.mailer
            .send(invitation_mail(&to, origin, &token))
            .await?;

        Ok(id)
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
            "You are invited to sign up to the Weaveling instance at {origin}. Follow this link within seven days to do so: {origin}/?invitation={}\n\nIf you did not expect this, you can ignore this mail.",
            token.as_str()
        ),
    }
}
