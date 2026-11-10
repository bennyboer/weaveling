use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing,
    Version,
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
            events: Arc::new(EventSourcingService::new(store.clone(), clock.clone())),
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

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(&id),
                    NoticeCommand::Note {
                        account: account.clone(),
                        about: about.clone(),
                    },
                    &Agent::System,
                )
                .await?;

            Ok(Some(id))
        })
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

    pub async fn dismiss(
        &self,
        notice: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<(), NotificationsError> {
        let id: NoticeId = notice.parse()?;
        self.read(notice, agent).await?;
        let key = AggregateId::from(&id);
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.events
                    .execute_at(transaction, &key, expected, NoticeCommand::Dismiss, agent)
                    .await?;
                Ok(())
            }
        )
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

    pub async fn remember_person(
        &self,
        within: Option<&mut Transaction>,
        person: &Person,
    ) -> Result<(), NotificationsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog.remember_person(transaction, person).await?;

            Ok(())
        })
    }

    pub async fn remember_project_name(
        &self,
        within: Option<&mut Transaction>,
        named: &ProjectName,
    ) -> Result<(), NotificationsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_project_name(transaction, named)
                .await?;

            Ok(())
        })
    }

    pub async fn lay_project_tombstone(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), NotificationsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .lay_project_tombstone(transaction, project, at)
                .await?;

            Ok(())
        })
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
            transacting::retrying!(&*self.transactions, |transaction| {
                self.store
                    .forget(transaction, &AggregateId::from(notice), KIND)
                    .await
                    .map_err(ServiceError::Store)?;
                self.catalog.forget(transaction, notice, now).await?;

                Ok::<_, NotificationsError>(())
            })?;
        }

        Ok(due.len())
    }

    /// Forgets every notice of a deleted account, whole, as an old one is, and its name.
    pub async fn forget_account(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
    ) -> Result<(), NotificationsError> {
        let notices = self.catalog.notices_of(account).await?;
        let now = self.clock.now();

        transacting::joining!(&*self.transactions, within, |transaction| {
            for notice in &notices {
                self.store
                    .forget(transaction, &AggregateId::from(notice), KIND)
                    .await
                    .map_err(ServiceError::Store)?;
                self.catalog.forget(transaction, notice, now).await?;
            }
            self.catalog.forget_person(transaction, account).await?;

            Ok(())
        })
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, NotificationsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let projects = self
                .catalog
                .forget_project_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let notices = self
                .catalog
                .forget_notice_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(projects + notices)
        })
    }

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        notice: &NoticeId,
    ) -> Result<(), NotificationsError> {
        let standing = match self.events.latest(&AggregateId::from(notice)).await {
            Ok(standing) => standing,
            Err(ServiceError::NotFound { .. }) => return Ok(()),
            Err(failure) => return Err(failure.into()),
        };

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember(
                    transaction,
                    &NoticeSummary::of(*notice, standing.version, &standing.state),
                )
                .await?;

            Ok(())
        })
    }
}

fn not_found(notice: &NoticeId) -> NotificationsError {
    NotificationsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(notice),
        kind: KIND,
    })
}
