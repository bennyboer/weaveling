use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing,
};
use time::{Duration, OffsetDateTime};
use transacting::{Transaction, Transactions};

use crate::catalog::{NoticeCatalog, NoticeSummary, Person, ProjectName};
use crate::error::NotificationsError;
use crate::id::NoticeId;
use crate::notice::{About, KIND, Notice, NoticeCommand, NoticeEvent, ProjectLink};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    pub people: HashMap<AccountId, String>,
    pub projects: HashMap<ProjectLink, String>,
}

pub const DISMISSED_KEPT_FOR: Duration = Duration::days(30);
pub const UNDISMISSED_KEPT_FOR: Duration = Duration::days(90);

#[derive(Clone)]
pub struct NotificationsCore {
    store: Arc<dyn EventStore<NoticeEvent>>,
    transactions: Arc<dyn Transactions>,
    events: Arc<EventSourcingService<Notice>>,
    catalog: Arc<dyn NoticeCatalog>,
    clock: Arc<dyn Clock>,
}

impl NotificationsCore {
    pub fn new(
        store: Arc<dyn EventStore<NoticeEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn NoticeCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(
                store.clone(),
                transactions.clone(),
                clock.clone(),
            )),
            store,
            transactions,
            catalog,
            clock,
        }
    }

    pub async fn note(
        &self,
        within: Option<&mut Transaction>,
        account: AccountId,
        about: About,
    ) -> Result<Option<NoticeId>, NotificationsError> {
        if let Some(project) = about.project()
            && self.catalog.has_project_tombstone(project).await?
        {
            return Ok(None);
        }
        let id = NoticeId::generate(self.clock.now());

        let mut transaction = self.transactions.join(within).await?;
        self.events
            .begin(
                Some(&mut transaction),
                &AggregateId::from(&id),
                NoticeCommand::Note { account, about },
                &Agent::System,
            )
            .await?;
        transaction.finish().await?;

        Ok(Some(id))
    }

    pub async fn read(
        &self,
        notice: &str,
        agent: &Agent,
    ) -> Result<Standing<Notice>, NotificationsError> {
        let id: NoticeId = notice.parse()?;
        let standing = self.events.latest(&AggregateId::from(&id)).await?;
        if !standing.state.is_for(agent) {
            return Err(not_found(&id));
        }

        Ok(standing)
    }

    pub async fn dismiss(&self, notice: &str, agent: &Agent) -> Result<(), NotificationsError> {
        let id: NoticeId = notice.parse()?;
        self.read(notice, agent).await?;
        transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .execute(
                    Some(transaction),
                    &AggregateId::from(&id),
                    NoticeCommand::Dismiss,
                    agent,
                )
                .await?;
            Ok(())
        })
    }

    pub async fn open_for(&self, agent: &Agent) -> Result<Vec<NoticeSummary>, NotificationsError> {
        match agent {
            Agent::Account(account) => Ok(self.catalog.open_for(account).await?),
            _ => Ok(Vec::new()),
        }
    }

    pub async fn names_in(&self, notices: &[NoticeSummary]) -> Result<Names, NotificationsError> {
        let accounts: Vec<AccountId> = notices
            .iter()
            .flat_map(|notice| notice.about.accounts())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let projects: Vec<ProjectLink> = notices
            .iter()
            .filter_map(|notice| notice.about.project())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        Ok(Names {
            people: self
                .catalog
                .people(&accounts)
                .await?
                .into_iter()
                .map(|person| (person.account, person.name))
                .collect(),
            projects: self
                .catalog
                .project_names(&projects)
                .await?
                .into_iter()
                .map(|named| (named.project, named.name))
                .collect(),
        })
    }

    pub async fn remember_person(&self, person: &Person) -> Result<(), NotificationsError> {
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .remember_person(&mut transaction, person)
            .await?;
        Ok(transaction.finish().await?)
    }

    pub async fn remember_project_name(
        &self,
        named: &ProjectName,
    ) -> Result<(), NotificationsError> {
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .remember_project_name(&mut transaction, named)
            .await?;
        Ok(transaction.finish().await?)
    }

    pub async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), NotificationsError> {
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .lay_project_tombstone(&mut transaction, project, at)
            .await?;
        Ok(transaction.finish().await?)
    }

    pub async fn forget_due(&self, at_most: usize) -> Result<usize, NotificationsError> {
        let now = self.clock.now();
        let due = self
            .catalog
            .due_for_forgetting(
                now - DISMISSED_KEPT_FOR,
                now - UNDISMISSED_KEPT_FOR,
                at_most,
            )
            .await?;
        for notice in &due {
            let mut transaction = self.transactions.begin().await?;
            self.store
                .forget(&mut transaction, &AggregateId::from(notice), KIND)
                .await
                .map_err(ServiceError::Store)?;
            self.catalog.forget(&mut transaction, notice).await?;
            transaction.finish().await?;
        }

        Ok(due.len())
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, NotificationsError> {
        let mut transaction = self.transactions.begin().await?;
        let forgotten = self
            .catalog
            .forget_project_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await?;

        Ok(forgotten)
    }

    pub async fn catalogue(&self, notice: &NoticeId) -> Result<(), NotificationsError> {
        let standing = self.events.latest(&AggregateId::from(notice)).await?;

        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .remember(
                &mut transaction,
                &NoticeSummary::of(*notice, standing.version, &standing.state),
            )
            .await?;
        Ok(transaction.finish().await?)
    }
}

fn not_found(notice: &NoticeId) -> NotificationsError {
    NotificationsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(notice),
        kind: KIND,
    })
}
