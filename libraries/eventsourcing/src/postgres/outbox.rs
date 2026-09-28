use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use messaging::{Conversation, Message, MessageId, Publisher, RoutingKey};
use serde_json::Value;
use sqlx::postgres::{PgListener, PgRow};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::outbox::{CLAIM_FOR, Delivered, Notifications, Outbox, OutboxError};

const CHANNEL: &str = "SELECT left('outbox_waiting_' || current_schema(), 63)";

const CLAIM: &str = "
    WITH waiting AS (
        SELECT entry
        FROM outbox
        WHERE published_at IS NULL AND (claimed_until IS NULL OR claimed_until < $1)
        ORDER BY entry
        LIMIT $3
        FOR UPDATE SKIP LOCKED
    )
    UPDATE outbox
    SET claimed_until = $2
    WHERE entry IN (SELECT entry FROM waiting)
    RETURNING entry, message_id, conversation, caused_by, routing_key, payload, occurred_at
";

const MARK_PUBLISHED: &str = "UPDATE outbox SET published_at = $2 WHERE entry = $1";

const DELETE_PUBLISHED: &str = "
    DELETE FROM outbox
    WHERE entry IN (
        SELECT entry
        FROM outbox
        WHERE published_at IS NOT NULL AND published_at < $1
        ORDER BY entry
        LIMIT $2
    )
";

pub struct PostgresOutbox {
    pool: PgPool,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
}

impl PostgresOutbox {
    pub fn new(pool: PgPool, publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            pool,
            publisher,
            clock,
        }
    }

    async fn listening(&self) -> Result<PgListener, OutboxError> {
        let channel: String = sqlx::query_scalar(CHANNEL)
            .fetch_one(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        let mut listening = PgListener::connect_with(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        listening
            .listen(&channel)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        Ok(listening)
    }

    async fn mark_published(&self, entry: i64) -> Result<(), OutboxError> {
        sqlx::query(MARK_PUBLISHED)
            .bind(entry)
            .bind(self.clock.now())
            .execute(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl Outbox for PostgresOutbox {
    async fn deliver(&self, at_most: i64) -> Result<Delivered, OutboxError> {
        let now = self.clock.now();
        let claimed = sqlx::query(CLAIM)
            .bind(now)
            .bind(now + CLAIM_FOR)
            .bind(at_most)
            .fetch_all(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        let mut delivered = Delivered {
            published: 0,
            refused: 0,
        };

        for row in &claimed {
            let (entry, message) = waiting(row)?;

            match self.publisher.publish(message).await {
                Ok(()) => {
                    self.mark_published(entry).await?;
                    delivered.published += 1;
                }
                Err(undelivered) => {
                    tracing::warn!(entry, error = %undelivered, "an outbox entry could not be published");
                    delivered.refused += 1;
                }
            }
        }

        Ok(delivered)
    }

    async fn delete_published(
        &self,
        before: OffsetDateTime,
        at_most: i64,
    ) -> Result<u64, OutboxError> {
        let gone = sqlx::query(DELETE_PUBLISHED)
            .bind(before)
            .bind(at_most)
            .execute(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        Ok(gone.rows_affected())
    }

    async fn notifications(&self) -> Result<Box<dyn Notifications>, OutboxError> {
        Ok(Box::new(self.listening().await?))
    }
}

#[async_trait]
impl Notifications for PgListener {
    async fn wait(&mut self) {
        if let Err(why) = self.recv().await {
            tracing::warn!(error = %why, "the outbox listener dropped, so delivery falls back to polling");
        }
    }
}

fn waiting(row: &PgRow) -> Result<(i64, Message), OutboxError> {
    let entry: i64 = column(row, "entry", 0)?;
    let id: Uuid = column(row, "message_id", entry)?;
    let conversation: Uuid = column(row, "conversation", entry)?;
    let caused_by: Option<Uuid> = column(row, "caused_by", entry)?;
    let routing: String = column(row, "routing_key", entry)?;
    let payload: Value = column(row, "payload", entry)?;
    let occurred_at: OffsetDateTime = column(row, "occurred_at", entry)?;

    let routing = RoutingKey::parse(&routing).map_err(|why| OutboxError::Unreadable {
        entry,
        why: why.to_string(),
    })?;

    Ok((
        entry,
        Message {
            id: MessageId::of(id),
            routing,
            conversation: Conversation::begun_by(MessageId::of(conversation)),
            caused_by: caused_by.map(MessageId::of),
            occurred_at,
            payload,
        },
    ))
}

fn column<'row, T>(row: &'row PgRow, named: &'static str, entry: i64) -> Result<T, OutboxError>
where
    T: sqlx::Decode<'row, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    row.try_get(named)
        .map_err(|failure| OutboxError::Unreadable {
            entry,
            why: format!("{named}: {failure}"),
        })
}
