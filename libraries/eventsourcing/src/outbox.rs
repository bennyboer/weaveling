use async_trait::async_trait;
use messaging::{Message, Notifications};
use thiserror::Error;
use time::{Duration, OffsetDateTime};

use crate::event::Recorded;
use crate::version::Version;

pub type MessageMapping<E> = fn(&Recorded<E>) -> Option<Message>;

#[derive(Debug, Clone, Copy)]
pub struct Origin<'a> {
    pub aggregate: &'a str,
    pub kind: &'a str,
    pub version: Version,
}

pub const CLAIM_FOR: Duration = Duration::seconds(30);

pub const KEPT_FOR: Duration = Duration::days(90);

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("the outbox could not be reached: {0}")]
    Unreachable(String),
    #[error("outbox entry {entry} holds something that is not a message: {why}")]
    Unreadable { entry: i64, why: String },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Delivered {
    pub published: usize,
    pub refused: usize,
}

#[async_trait]
pub trait Outbox: Send + Sync {
    async fn deliver(&self, at_most: i64) -> Result<Delivered, OutboxError>;

    async fn delete_published(
        &self,
        before: OffsetDateTime,
        at_most: i64,
    ) -> Result<u64, OutboxError>;

    async fn notifications(&self) -> Result<Box<dyn Notifications>, OutboxError>;
}
