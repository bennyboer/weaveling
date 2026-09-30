use std::sync::Arc;

use clock::Clock;
use eventpublishing::{UnreadableMessage, published_in};
use eventsourcing::{Agent, ServiceError};
use messaging::{
    Delivery, Listener, ListenerName, Message, NotHandled, Publisher, RoutingKey, Subscription,
    Undelivered,
};
use pieces_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use pieces_core::{
    CatalogError, PieceCatalog, PieceError, PieceId, PieceService, PieceServiceError, ProjectLink,
};
use projects_contract::{DELETED, ProjectEventDTO};
use thiserror::Error;

const NAME: &str = "discard-pieces-of-deleted-project";

pub const AT_MOST: usize = 100;

pub struct DiscardOnProjectDeleted {
    pieces: PieceService,
    catalog: Arc<dyn PieceCatalog>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

#[derive(Debug, Error)]
enum NotSwept {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error("this message does not say which project to sweep: {0}")]
    Unshaped(String),
    #[error("this message names something that is not a piece")]
    NotAPieceId(#[source] ids::InvalidId),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Pieces(#[from] PieceServiceError),
    #[error(transparent)]
    Undelivered(#[from] Undelivered),
}

pub fn when_project_deleted() -> Subscription {
    Subscription::parse(DELETED).expect("a declared routing key holds no wildcards")
}

pub fn when_more_to_sweep() -> Subscription {
    Subscription::parse(MORE_TO_SWEEP).expect("a declared routing key holds no wildcards")
}

impl DiscardOnProjectDeleted {
    pub fn new(
        pieces: PieceService,
        catalog: Arc<dyn PieceCatalog>,
        publisher: Arc<dyn Publisher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            pieces,
            catalog,
            publisher,
            clock,
            at_most: AT_MOST,
        }
    }

    pub fn taking_at_most(mut self, at_most: usize) -> Self {
        self.at_most = at_most;
        self
    }

    async fn sweep(&self, asked: &Message) -> Result<(), NotSwept> {
        let (project, after) = what_to_sweep(asked)?;
        let batch = self
            .catalog
            .in_project_after(&project, after, self.at_most)
            .await?;

        let Some(last) = batch.last().map(|summary| summary.id) else {
            return Ok(());
        };

        for summary in &batch {
            match self
                .pieces
                .discard(&summary.id.to_string(), None, &nobody())
                .await
            {
                Ok(_) => {}
                Err(refused) if already_discarded(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        if batch.len() < self.at_most {
            return Ok(());
        }

        self.publisher
            .publish(self.carry_on(asked, &project, last))
            .await?;

        Ok(())
    }

    fn carry_on(&self, asked: &Message, project: &ProjectLink, after: PieceId) -> Message {
        asked.answering(
            RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key holds no wildcards"),
            serde_json::to_value(MoreToSweepDTO {
                project: project.to_string(),
                after: after.to_string(),
            })
            .expect("a continuation is plain data and cannot fail to serialize"),
            self.clock.now(),
        )
    }
}

fn what_to_sweep(asked: &Message) -> Result<(ProjectLink, Option<PieceId>), NotSwept> {
    if asked.routing.to_string() == MORE_TO_SWEEP {
        let carried: MoreToSweepDTO = serde_json::from_value(asked.payload.clone())
            .map_err(|why| NotSwept::Unshaped(why.to_string()))?;

        return Ok((
            ProjectLink::from(carried.project.as_str()),
            Some(carried.after.parse().map_err(NotSwept::NotAPieceId)?),
        ));
    }

    let deleted = published_in::<ProjectEventDTO>(asked)?;

    Ok((ProjectLink::from(deleted.aggregate.id.as_str()), None))
}

fn already_discarded(refused: &PieceServiceError) -> bool {
    matches!(
        refused,
        PieceServiceError::Events(ServiceError::Refused(PieceError::Discarded))
    )
}

fn nobody() -> Agent {
    Agent::System
}

#[async_trait::async_trait]
impl Listener for DiscardOnProjectDeleted {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the sweep listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_project_deleted(), when_more_to_sweep()]
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.sweep(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
