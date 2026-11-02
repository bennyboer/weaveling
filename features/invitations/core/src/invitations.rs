use std::sync::Arc;

use acting::Acting;
use authorizing::{Act, Authorizer};
use clock::Clock;
use email::EmailAddress;
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, ServiceError, Version};
use mailing::Mailer;
use throttling::Throttles;
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::catalog::{
    InvitationCatalog, InvitationSummary, InvitationToYou, ListedInvitation, Person, ProjectName,
    Status,
};
use crate::error::InvitationsError;
use crate::id::InvitationId;
use crate::invitation::{DeclineReason, InvitationError, InvitationEvent, KIND};
use crate::scope::{ProjectLink, Role, Scope};
use crate::service::InvitationService;
use crate::token::{InvitationToken, TokenKeys};

#[derive(Clone)]
pub struct InvitationsCore {
    service: InvitationService,
    transactions: Arc<dyn Transactions>,
    keys: Arc<dyn TokenKeys>,
    catalog: Arc<dyn InvitationCatalog>,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl InvitationsCore {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<dyn EventStore<InvitationEvent>>,
        transactions: Arc<dyn Transactions>,
        keys: Arc<dyn TokenKeys>,
        throttles: Arc<dyn Throttles>,
        catalog: Arc<dyn InvitationCatalog>,
        mailer: Arc<dyn Mailer>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: InvitationService::new(store, throttles, mailer, clock.clone()),
            transactions,
            keys,
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

        self.issued(to, origin, Scope::Instance, true, &by.agent)
            .await
    }

    async fn issued(
        &self,
        to: &str,
        origin: &str,
        scope: Scope,
        by_an_operator: bool,
        agent: &Agent,
    ) -> Result<InvitationSummary, InvitationsError> {
        let to = EmailAddress::new(to)?;
        self.service.admitted(&to).await?;
        let key = self.keys.key().await?;

        let mut transaction = self.transactions.begin().await?;
        let summary = self
            .service
            .issue(
                &mut transaction,
                &key,
                &to,
                origin,
                scope,
                by_an_operator,
                agent,
            )
            .await?;
        transaction.finish().await?;

        Ok(summary)
    }

    pub async fn mail_issued(&self, invitation: &InvitationId) -> Result<(), InvitationsError> {
        self.service
            .mail_issued(&self.keys.key().await?, invitation)
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

        self.issued(
            to,
            origin,
            Scope::Project { project, role },
            by.operator,
            &by.agent,
        )
        .await
    }

    pub async fn address_of(
        &self,
        agent: &Agent,
    ) -> Result<Option<EmailAddress>, InvitationsError> {
        let Agent::Account(account) = agent else {
            return Ok(None);
        };

        Ok(self
            .catalog
            .people(std::slice::from_ref(account))
            .await?
            .into_iter()
            .next()
            .map(|you| you.email))
    }

    pub async fn invitations_to(
        &self,
        by: &Acting,
    ) -> Result<Vec<InvitationToYou>, InvitationsError> {
        let Some(address) = self.address_of(&by.agent).await? else {
            return Ok(Vec::new());
        };
        let now = self.clock.now();
        let open: Vec<(InvitationSummary, ProjectLink, Role)> = self
            .catalog
            .open_to(&address)
            .await?
            .into_iter()
            .filter(|summary| summary.status(now) == Some(Status::Open))
            .filter_map(|summary| match summary.scope.clone() {
                Scope::Project { project, role } => Some((summary, project, role)),
                Scope::Instance => None,
            })
            .collect();
        let mut waiting = Vec::new();
        for (summary, project, role) in open {
            if !self.catalog.is_in_the_trash(&project).await? {
                waiting.push((summary, project, role));
            }
        }
        let open = waiting;
        let projects: Vec<ProjectLink> =
            open.iter().map(|(_, project, _)| project.clone()).collect();
        let names = self.catalog.project_names(&projects).await?;
        let issuers: Vec<AccountId> = open
            .iter()
            .filter_map(|(summary, _, _)| summary.issued_by.clone())
            .collect();
        let inviters = self.catalog.people(&issuers).await?;

        Ok(open
            .into_iter()
            .map(|(summary, project, role)| InvitationToYou {
                id: summary.id,
                project_name: names
                    .iter()
                    .find(|named| named.project == project)
                    .map(|named| named.name.clone()),
                invited_by: inviters
                    .iter()
                    .find(|person| Some(&person.account) == summary.issued_by.as_ref())
                    .map(|person| person.name.clone()),
                project,
                role,
                until: summary.until,
            })
            .collect())
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

        transacting::retrying!(&*self.transactions, |transaction| {
            self.service
                .withdraw(transaction, invitation, &by.agent)
                .await
        })
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
        within: Option<&mut Transaction>,
        invitation: &InvitationId,
        at: OffsetDateTime,
        by: &Agent,
    ) -> Result<(), InvitationsError> {
        let mut transaction = self.transactions.join(within).await?;
        settled(
            self.service
                .accept(&mut transaction, invitation, at, by)
                .await,
        )?;
        Ok(transaction.finish().await?)
    }

    pub async fn declined(
        &self,
        within: Option<&mut Transaction>,
        invitation: &InvitationId,
        reason: DeclineReason,
    ) -> Result<(), InvitationsError> {
        let mut transaction = self.transactions.join(within).await?;
        settled(
            self.service
                .decline(&mut transaction, invitation, reason, &Agent::System)
                .await,
        )?;
        Ok(transaction.finish().await?)
    }

    pub async fn decline_by_token(&self, token: &str) -> Result<(), InvitationsError> {
        let Some(summary) = self
            .catalog
            .with_digest(&InvitationToken::presented(token).digest())
            .await?
        else {
            return Err(InvitationsError::InvitationInvalid);
        };
        if summary.status(self.clock.now()) == Some(Status::Expired) {
            return Err(InvitationsError::InvitationExpired);
        }

        match transacting::retrying!(&*self.transactions, |transaction| {
            self.service
                .decline(
                    transaction,
                    &summary.id,
                    DeclineReason::TurnedDown,
                    &Agent::System,
                )
                .await
        }) {
            Err(InvitationsError::Events(ServiceError::Refused(
                InvitationError::AlreadyAccepted | InvitationError::Withdrawn,
            ))) => Err(InvitationsError::InvitationInvalid),
            declined => declined,
        }
    }

    pub async fn remember_project_name(
        &self,
        within: Option<&mut Transaction>,
        named: &ProjectName,
    ) -> Result<(), InvitationsError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .remember_project_name(&mut transaction, named)
            .await?;
        Ok(transaction.finish().await?)
    }

    pub async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), InvitationsError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .remember_trash(&mut transaction, project, version, in_the_trash)
            .await?;
        Ok(transaction.finish().await?)
    }

    pub async fn lay_project_tombstone(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), InvitationsError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .lay_project_tombstone(&mut transaction, project, at)
            .await?;
        Ok(transaction.finish().await?)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, InvitationsError> {
        let mut transaction = self.transactions.begin().await?;
        let projects = self
            .catalog
            .forget_project_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        let invitations = self
            .catalog
            .forget_invitation_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await?;

        Ok(projects + invitations)
    }

    pub async fn remember_person(
        &self,
        within: Option<&mut Transaction>,
        person: &Person,
    ) -> Result<(), InvitationsError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .remember_person(&mut transaction, person)
            .await?;
        Ok(transaction.finish().await?)
    }

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        invitation: &InvitationId,
    ) -> Result<Option<ProjectLink>, InvitationsError> {
        let standing = self.service.latest(invitation).await?;
        let summary = InvitationSummary::of(*invitation, standing.version, &standing.state);
        let mut transaction = self.transactions.join(within).await?;
        if standing.state.is_retired() {
            self.catalog
                .forget(&mut transaction, invitation, self.clock.now())
                .await?;
            transaction.finish().await?;
            return Ok(None);
        }
        self.catalog.remember(&mut transaction, &summary).await?;
        transaction.finish().await?;

        Ok(summary.scope.project().cloned())
    }

    pub async fn withdraw_if_project_gone(
        &self,
        within: Option<&mut Transaction>,
        invitation: &InvitationId,
        project: &ProjectLink,
    ) -> Result<(), InvitationsError> {
        let mut transaction = self.transactions.join(within).await?;
        if self.catalog.has_project_tombstone(project).await? {
            settled(
                self.service
                    .withdraw(&mut transaction, invitation, &Agent::System)
                    .await,
            )?;
        }

        Ok(transaction.finish().await?)
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
                settled(self.withdrawn_by_the_system(&summary.id).await)?;
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
            transacting::retrying!(&*self.transactions, |transaction| {
                self.service
                    .retire(transaction, invitation, &Agent::System)
                    .await
            })?;
        }

        Ok(due.len())
    }

    async fn withdrawn_by_the_system(
        &self,
        invitation: &InvitationId,
    ) -> Result<(), InvitationsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            self.service
                .withdraw(transaction, invitation, &Agent::System)
                .await
        })
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
