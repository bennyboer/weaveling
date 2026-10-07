use std::future::pending;
use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use clock::text;
use messaging::{Conversation, Message, MessageId, Notifications, Publisher, RoutingKey};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::outbox::{CLAIM_FOR, Delivered, Outbox, OutboxError};

const CLAIM: &str = "
    UPDATE outbox
    SET claimed_until = ?2
    WHERE entry IN (
        SELECT entry
        FROM outbox
        WHERE published_at IS NULL AND (claimed_until IS NULL OR claimed_until < ?1)
        ORDER BY entry
        LIMIT ?3
    )
    RETURNING entry, message_id, conversation, caused_by, routing_key, payload, occurred_at
";

const MARK_PUBLISHED: &str = "UPDATE outbox SET published_at = ?2 WHERE entry = ?1";

const DELETE_PUBLISHED: &str = "
    DELETE FROM outbox
    WHERE entry IN (
        SELECT entry
        FROM outbox
        WHERE published_at IS NOT NULL AND published_at < ?1
        ORDER BY entry
        LIMIT ?2
    )
";

pub struct SqliteOutbox {
    pool: SqlitePool,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
}

struct Unannounced;

#[async_trait]
impl Notifications for Unannounced {
    async fn wait(&mut self) {
        pending::<()>().await;
    }
}

impl SqliteOutbox {
    pub fn new(pool: SqlitePool, publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            pool,
            publisher,
            clock,
        }
    }

    async fn mark_published(&self, entry: i64) -> Result<(), OutboxError> {
        sqlx::query(MARK_PUBLISHED)
            .bind(entry)
            .bind(text::written(self.clock.now()))
            .execute(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl Outbox for SqliteOutbox {
    async fn deliver(&self, at_most: i64) -> Result<Delivered, OutboxError> {
        let now = self.clock.now();
        let claimed = sqlx::query(CLAIM)
            .bind(text::written(now))
            .bind(text::written(now + CLAIM_FOR))
            .bind(at_most)
            .fetch_all(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        let mut waiting = claimed.iter().map(waiting).collect::<Result<Vec<_>, _>>()?;
        waiting.sort_by_key(|(entry, _)| *entry);

        let mut delivered = Delivered {
            published: 0,
            refused: 0,
        };

        for (entry, message) in waiting {
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
            .bind(text::written(before))
            .bind(at_most)
            .execute(&self.pool)
            .await
            .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;

        Ok(gone.rows_affected())
    }

    async fn notifications(&self) -> Result<Box<dyn Notifications>, OutboxError> {
        Ok(Box::new(Unannounced))
    }
}

fn waiting(row: &SqliteRow) -> Result<(i64, Message), OutboxError> {
    let entry: i64 = column(row, "entry", 0)?;
    let id: String = column(row, "message_id", entry)?;
    let conversation: String = column(row, "conversation", entry)?;
    let caused_by: Option<String> = column(row, "caused_by", entry)?;
    let routing: String = column(row, "routing_key", entry)?;
    let payload: String = column(row, "payload", entry)?;
    let occurred_at: String = column(row, "occurred_at", entry)?;

    let unreadable = |why: String| OutboxError::Unreadable { entry, why };
    let uuid = |named: &str, text: &str| {
        Uuid::parse_str(text).map_err(|why| unreadable(format!("{named}: {why}")))
    };

    Ok((
        entry,
        Message {
            id: MessageId::of(uuid("message_id", &id)?),
            routing: RoutingKey::parse(&routing).map_err(|why| unreadable(why.to_string()))?,
            conversation: Conversation::begun_by(MessageId::of(uuid(
                "conversation",
                &conversation,
            )?)),
            caused_by: caused_by
                .map(|caused| uuid("caused_by", &caused).map(MessageId::of))
                .transpose()?,
            occurred_at: text::read(&occurred_at)
                .ok_or_else(|| unreadable(format!("occurred_at: {occurred_at}")))?,
            payload: serde_json::from_str(&payload)
                .map_err(|why| unreadable(format!("payload: {why}")))?,
        },
    ))
}

fn column<'row, T>(row: &'row SqliteRow, named: &'static str, entry: i64) -> Result<T, OutboxError>
where
    T: sqlx::Decode<'row, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    row.try_get(named)
        .map_err(|failure| OutboxError::Unreadable {
            entry,
            why: format!("{named}: {failure}"),
        })
}
