use async_trait::async_trait;
use messaging::{Message, Notifications};
use serde::Serialize;
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use transacting::{Failure, Transaction};

#[derive(Debug, Clone, Copy)]
pub struct Origin<'a> {
    pub aggregate: &'a str,
    pub kind: &'a str,
    pub version: Option<u64>,
}

impl<'a> Origin<'a> {
    pub fn unversioned(aggregate: &'a str, kind: &'a str) -> Self {
        Self {
            aggregate,
            kind,
            version: None,
        }
    }
}

pub const CLAIM_FOR: Duration = Duration::seconds(30);

pub const KEPT_FOR: Duration = Duration::days(90);

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("the outbox could not be reached: {0}")]
    Unreachable(String),
    #[error("outbox entry {entry} holds something that is not a message: {why}")]
    Unreadable { entry: i64, why: String },
    #[error(transparent)]
    Transacting(#[from] Failure),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Delivered {
    pub published: usize,
    pub refused: usize,
}

#[async_trait]
pub trait Outbox: Send + Sync {
    async fn enqueue_within(
        &self,
        transaction: &mut Transaction,
        origin: Origin<'_>,
        message: &Message,
    ) -> Result<(), OutboxError>;

    async fn deliver(&self, at_most: i64) -> Result<Delivered, OutboxError>;

    async fn delete_published(
        &self,
        before: OffsetDateTime,
        at_most: i64,
    ) -> Result<u64, OutboxError>;

    async fn notifications(&self) -> Result<Box<dyn Notifications>, OutboxError>;
}

impl dyn Outbox {
    pub async fn enqueue_follow_up_within(
        &self,
        transaction: &mut Transaction,
        origin: Origin<'_>,
        heard: &Message,
        routing: &str,
        payload: &(impl Serialize + Sync),
        now: OffsetDateTime,
    ) -> Result<(), OutboxError> {
        self.enqueue_within(transaction, origin, &heard.follow_up(routing, payload, now))
            .await
    }
}
