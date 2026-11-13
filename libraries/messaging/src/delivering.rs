use async_trait::async_trait;
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::listening::{ListenerName, Notifications};
use crate::message::Message;

pub const ATTEMPTS: i32 = 5;

pub const CLAIM_FOR: Duration = Duration::seconds(30);

pub const BACKOFF: Duration = Duration::milliseconds(200);

pub const SLOWER_BY: i32 = 5;

pub const RETRIABLE_FOR: Duration = Duration::days(30);

#[derive(Debug, Error)]
pub enum DeliveryError {
    #[error("the deliveries could not be reached: {0}")]
    Unreachable(String),
    #[error("delivery {delivery} holds something that is not a message: {why}")]
    Unreadable { delivery: i64, why: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Claim(Uuid);

impl Claim {
    pub fn drawn() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn of(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub id: i64,
    pub claim: Claim,
    pub listener: ListenerName,
    pub message: Message,
    pub attempts: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadLetter {
    pub id: i64,
    pub listener: ListenerName,
    pub message: Message,
    pub attempts: i32,
    pub why: String,
    pub given_up_at: OffsetDateTime,
    pub acknowledged_at: Option<OffsetDateTime>,
}

impl DeadLetter {
    pub fn may_be_tried_again(&self, now: OffsetDateTime) -> bool {
        now - self.message.occurred_at <= RETRIABLE_FOR
    }
}

#[async_trait]
pub trait Deliveries: Send + Sync {
    async fn enqueue(
        &self,
        listener: &ListenerName,
        message: &Message,
    ) -> Result<(), DeliveryError>;

    async fn claim_due(
        &self,
        now: OffsetDateTime,
        at_most: i64,
    ) -> Result<Vec<Queued>, DeliveryError>;

    async fn mark_as_handled(&self, queued: &Queued) -> Result<(), DeliveryError>;

    async fn mark_as_refused(
        &self,
        queued: &Queued,
        why: &str,
        again_at: OffsetDateTime,
    ) -> Result<(), DeliveryError>;

    async fn give_up(
        &self,
        queued: &Queued,
        why: &str,
        at: OffsetDateTime,
    ) -> Result<(), DeliveryError>;

    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, DeliveryError>;

    async fn retry(&self, dead_letter: i64, at: OffsetDateTime) -> Result<(), DeliveryError>;

    async fn acknowledge(&self, dead_letter: i64, at: OffsetDateTime) -> Result<(), DeliveryError>;

    /// Forgets old dead letters, whose message happened before `before`.
    /// It forgets at most `at_most` of each kind.
    ///
    /// A dead letter this old is too old to be retried.
    /// It is only kept to record what it was about.
    /// An acknowledged one is deleted completely, since its alarm has been noticed.
    /// An unacknowledged one only loses its payload.
    /// The letter itself is kept for whoever reads why it was refused.
    async fn forget_old_dead_letters(
        &self,
        before: OffsetDateTime,
        at_most: i64,
    ) -> Result<u64, DeliveryError>;

    async fn waiting(&self) -> Result<usize, DeliveryError>;

    async fn notifications(&self) -> Result<Box<dyn Notifications>, DeliveryError>;
}

pub fn again_after(attempts: i32) -> Duration {
    BACKOFF * SLOWER_BY.pow((attempts.max(1) - 1).min(ATTEMPTS - 1) as u32)
}
