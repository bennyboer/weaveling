use std::sync::Arc;

use clock::Clock;
use email::EmailAddress;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, Standing, Version};
use mailing::{Mail, Mailer};
use throttling::{Admission, Rule, Throttles};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::catalog::InvitationSummary;
use crate::error::InvitationsError;
use crate::id::InvitationId;
use crate::invitation::{DeclineReason, Invitation, InvitationCommand, InvitationEvent, Outcome};
use crate::scope::{Role, Scope};
use crate::token::InvitationToken;

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
        transactions: Arc<dyn Transactions>,
        throttles: Arc<dyn Throttles>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(
                store,
                transactions,
                clock.clone(),
            )),
            throttles,
            mailer,
            clock,
        }
    }

    pub async fn issue(
        &self,
        transaction: &mut Transaction,
        to: &EmailAddress,
        origin: &str,
        scope: Scope,
        by_an_operator: bool,
        agent: &Agent,
    ) -> Result<(InvitationSummary, Mail), InvitationsError> {
        let now = self.clock.now();
        let id = InvitationId::generate(now);
        let until = now + Invitation::LASTS;
        let token = InvitationToken::drawn();
        self.events
            .begin(
                Some(transaction),
                &AggregateId::from(&id),
                InvitationCommand::Issue {
                    to: to.clone(),
                    digest: token.digest(),
                    until,
                    scope: scope.clone(),
                    by_an_operator,
                },
                agent,
            )
            .await?;
        let mail = invitation_mail(to, origin, &scope, &token);

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

        Ok((summary, mail))
    }

    pub async fn send(&self, mail: Mail) -> Result<(), InvitationsError> {
        Ok(self.mailer.send(mail).await?)
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
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(transaction, invitation, InvitationCommand::Withdraw, agent)
            .await
    }

    pub async fn retire(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(transaction, invitation, InvitationCommand::Retire, agent)
            .await
    }

    pub async fn accept(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        at: OffsetDateTime,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(
            transaction,
            invitation,
            InvitationCommand::Accept { at },
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
            agent,
        )
        .await
    }

    async fn carry_out(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        command: InvitationCommand,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        let _landed = self
            .events
            .execute(
                Some(transaction),
                &AggregateId::from(invitation),
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
    let origin = origin.trim().trim_end_matches('/');
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
