use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing,
    StoreError,
};
use registry::Registry;

use crate::catalog::{NoticeCatalog, NoticeSummary};
use crate::error::NotificationsError;
use crate::id::NoticeId;
use crate::notice::{About, KIND, Notice, NoticeCommand, NoticeError, NoticeEvent};

#[derive(Clone)]
pub struct NotificationsCore {
    events: Arc<EventSourcingService<Notice>>,
    catalog: Arc<dyn NoticeCatalog>,
    registry: Arc<dyn Registry>,
    clock: Arc<dyn Clock>,
}

impl NotificationsCore {
    pub fn new(
        store: Arc<dyn EventStore<NoticeEvent>>,
        catalog: Arc<dyn NoticeCatalog>,
        registry: Arc<dyn Registry>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            catalog,
            registry,
            clock,
        }
    }

    pub async fn note(
        &self,
        cause: &str,
        account: AccountId,
        about: About,
    ) -> Result<NoticeId, NotificationsError> {
        let mine = NoticeId::generate(self.clock.now());
        let id: NoticeId = self
            .registry
            .claim(KIND.as_str(), cause, &mine.to_string())
            .await?
            .parse()?;

        match self
            .events
            .begin(
                &AggregateId::from(&id),
                NoticeCommand::Note { account, about },
                &Agent::System,
            )
            .await
        {
            Ok(_)
            | Err(ServiceError::Refused(NoticeError::AlreadyNoted))
            | Err(ServiceError::Store(StoreError::Outdated { .. })) => Ok(id),
            Err(failed) => Err(failed.into()),
        }
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
        self.events
            .execute(&AggregateId::from(&id), NoticeCommand::Dismiss, agent)
            .await?;

        Ok(())
    }

    pub async fn open_for(&self, agent: &Agent) -> Result<Vec<NoticeSummary>, NotificationsError> {
        match agent {
            Agent::Account(account) => Ok(self.catalog.open_for(account).await?),
            _ => Ok(Vec::new()),
        }
    }

    pub async fn catalogue(&self, notice: &NoticeId) -> Result<(), NotificationsError> {
        let standing = self.events.latest(&AggregateId::from(notice)).await?;

        Ok(self
            .catalog
            .remember(&NoticeSummary::of(
                *notice,
                standing.version,
                &standing.state,
            ))
            .await?)
    }
}

fn not_found(notice: &NoticeId) -> NotificationsError {
    NotificationsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(notice),
        kind: KIND,
    })
}
