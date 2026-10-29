use std::sync::Arc;

use clock::Clock;
use email::EmailAddress;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, Standing, Version};
use mailing::{Mail, Mailer};
use throttling::{Rule, Throttles};
use time::OffsetDateTime;

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
        to: &str,
        origin: &str,
        scope: Scope,
        by_an_operator: bool,
        agent: &Agent,
    ) -> Result<InvitationSummary, InvitationsError> {
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
                    scope: scope.clone(),
                    by_an_operator,
                },
                agent,
            )
            .await?;
        self.mailer
            .send(invitation_mail(&to, origin, &scope, &token))
            .await?;

        Ok(InvitationSummary {
            id,
            version: Version::of(1),
            to,
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
        })
    }

    pub async fn latest(
        &self,
        invitation: &InvitationId,
    ) -> Result<Standing<Invitation>, InvitationsError> {
        Ok(self.events.latest(&AggregateId::from(invitation)).await?)
    }

    pub async fn withdraw(
        &self,
        invitation: &InvitationId,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(invitation, InvitationCommand::Withdraw, agent)
            .await
    }

    pub async fn retire(
        &self,
        invitation: &InvitationId,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(invitation, InvitationCommand::Retire, agent)
            .await
    }

    pub async fn accept(
        &self,
        invitation: &InvitationId,
        at: OffsetDateTime,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(invitation, InvitationCommand::Accept { at }, agent)
            .await
    }

    pub async fn decline(
        &self,
        invitation: &InvitationId,
        reason: DeclineReason,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        self.carry_out(invitation, InvitationCommand::Decline { reason }, agent)
            .await
    }

    async fn carry_out(
        &self,
        invitation: &InvitationId,
        command: InvitationCommand,
        agent: &Agent,
    ) -> Result<(), InvitationsError> {
        let _landed = self
            .events
            .execute(&AggregateId::from(invitation), command, agent)
            .await?;

        Ok(())
    }

    async fn not_throttled(&self, to: &EmailAddress) -> Result<(), InvitationsError> {
        let now = self.clock.now();
        let Some(tries) = self.throttles.tries(INVITING, to.as_str()).await? else {
            return Ok(());
        };
        if tries.is_forgotten(now) {
            return Ok(self.throttles.clear(INVITING, to.as_str()).await?);
        }

        match tries.wait_before_next(INVITING_RULE, now) {
            Some(wait) => Err(InvitationsError::Throttled { wait }),
            None => Ok(()),
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
