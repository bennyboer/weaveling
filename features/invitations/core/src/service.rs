use std::sync::Arc;

use clock::Clock;
use email::EmailAddress;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, Standing, Version};
use mailing::{Mail, Mailer};
use throttling::{Admission, Rule, Throttles};
use transacting::Transaction;

use crate::catalog::InvitationSummary;
use crate::error::InvitationsError;
use crate::id::InvitationId;
use crate::invitation::{DeclineReason, Invitation, InvitationCommand, InvitationEvent, Outcome};
use crate::scope::{Role, Scope};
use crate::token::{InvitationToken, TokenKey};

const INVITING: &str = "inviting";
const INVITING_RULE: Rule = Rule { free: 3 };

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

    #[allow(clippy::too_many_arguments)]
    pub async fn issue(
        &self,
        transaction: &mut Transaction,
        key: &TokenKey,
        to: &EmailAddress,
        origin: &str,
        scope: Scope,
        by_an_operator: bool,
        agent: &Agent,
    ) -> Result<InvitationSummary, InvitationsError> {
        let now = self.clock.now();
        let id = InvitationId::generate(now);
        let until = now + Invitation::LASTS;
        let token = InvitationToken::derived(key, &id);
        self.events
            .begin(
                transaction,
                &AggregateId::from(&id),
                InvitationCommand::Issue {
                    to: to.clone(),
                    digest: token.digest(),
                    until,
                    scope: scope.clone(),
                    by_an_operator,
                    origin: origin.trim().trim_end_matches('/').to_owned(),
                },
                agent,
            )
            .await?;

        let summary = InvitationSummary {
            id,
            version: Version::of(1),
            to: to.clone(),
            digest: token.digest(),
            scope,
            issued_by: match agent {
                Agent::Account(account) => Some(account.clone()),
                _ => None,
            },
            issued_at: now,
            until,
            outcome: Outcome::Open,
            retire_after: until + InvitationSummary::EXPIRED_LISTED_FOR,
        };

        Ok(summary)
    }

    pub async fn mail_issued(
        &self,
        key: &TokenKey,
        invitation: &InvitationId,
    ) -> Result<(), InvitationsError> {
        let issued = self.latest(invitation).await?.state;
        if issued.outcome() != Outcome::Open || issued.is_expired(self.clock.now()) {
            return Ok(());
        }

        Ok(self
            .mailer
            .send(invitation_mail(
                issued.to(),
                issued.origin(),
                issued.scope(),
                &InvitationToken::derived(key, invitation),
            ))
            .await?)
    }

    pub async fn latest(
        &self,
        invitation: &InvitationId,
    ) -> Result<Standing<Invitation>, InvitationsError> {
        Ok(self.events.latest(&AggregateId::from(invitation)).await?)
    }

    pub async fn withdraw(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(
            transaction,
            invitation,
            InvitationCommand::Withdraw,
            expected,
            agent,
        )
        .await
    }

    pub async fn retire(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(
            transaction,
            invitation,
            InvitationCommand::Retire,
            None,
            agent,
        )
        .await?;
        let aggregate = AggregateId::from(invitation);
        self.events.compact(transaction, &aggregate, agent).await?;
        self.events
            .forget_published(transaction, &aggregate)
            .await?;

        Ok(())
    }

    pub async fn accept(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(
            transaction,
            invitation,
            InvitationCommand::Accept,
            None,
            agent,
        )
        .await
    }

    pub async fn decline(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        reason: DeclineReason,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(
            transaction,
            invitation,
            InvitationCommand::Decline { reason },
            None,
            agent,
        )
        .await
    }

    async fn carry_out(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        command: InvitationCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.events
            .execute_at(
                transaction,
                &AggregateId::from(invitation),
                expected,
                command,
                agent,
            )
            .await?;

        Ok(())
    }

    pub async fn admitted(&self, to: &EmailAddress) -> Result<(), InvitationsError> {
        match self
            .throttles
            .admit(INVITING, to.as_str(), INVITING_RULE, self.clock.now())
            .await?
        {
            Admission::Admitted => Ok(()),
            Admission::Wait(wait) => Err(InvitationsError::Throttled { wait }),
        }
    }
}

fn invitation_mail(
    to: &EmailAddress,
    origin: &str,
    scope: &Scope,
    token: &InvitationToken,
) -> Mail {
    let (subject, invited) = match scope {
        Scope::Instance => (
            "You are invited to Weaveling".to_owned(),
            format!(
                "You are invited to sign up to the Weaveling instance at {origin}. Follow this link within seven days to do so"
            ),
        ),
        Scope::Project { role, .. } => (
            "You are invited to a project on Weaveling".to_owned(),
            format!(
                "You are invited to work on a project as {} on the Weaveling instance at {origin}. Follow this link within seven days to join it",
                as_whom(*role)
            ),
        ),
    };

    Mail {
        to: to.to_string(),
        subject,
        body: format!(
            "{invited}: {origin}/invitation#{}\n\nIf you did not expect this, you can ignore this mail.",
            token.as_str()
        ),
    }
}

fn as_whom(role: Role) -> &'static str {
    match role {
        Role::Owner => "an owner",
        Role::Author => "an author",
        Role::Reader => "a reader",
    }
}
