use std::sync::Arc;

use async_trait::async_trait;
use clock::text;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::delivering::{CLAIM_FOR, DeadLetter, Deliveries, DeliveryError, Queued};
use crate::listening::{ListenerName, Notifications};
use crate::message::{Conversation, Message, MessageId};
use crate::routing::RoutingKey;

const LEDGER: &str = "_sqlx_migrations_deliveries";

const ENQUEUE: &str = "
    INSERT INTO deliveries
        (listener, message_id, conversation, caused_by, routing_key, payload, occurred_at, due_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
";

const CLAIM_DUE: &str = "
    UPDATE deliveries
    SET claimed_until = ?2
    WHERE delivery IN (
        SELECT delivery
        FROM deliveries
        WHERE due_at <= ?1 AND (claimed_until IS NULL OR claimed_until < ?1)
        ORDER BY delivery
        LIMIT ?3
    )
    RETURNING delivery, listener, message_id, conversation, caused_by, routing_key, payload,
              occurred_at, attempts
";

const HANDLED: &str = "DELETE FROM deliveries WHERE delivery = ?1";

const REFUSED: &str = "
    UPDATE deliveries
    SET attempts = attempts + 1, due_at = ?2, claimed_until = NULL, last_refusal = ?3
    WHERE delivery = ?1
";

const BURY: &str = "
    INSERT INTO dead_letters
        (listener, message_id, conversation, caused_by, routing_key, payload, occurred_at,
         attempts, why)
    SELECT listener, message_id, conversation, caused_by, routing_key, payload, occurred_at,
           attempts + 1, ?2
    FROM deliveries
    WHERE delivery = ?1
";

const DEAD_LETTERS: &str = "
    SELECT listener, message_id, conversation, caused_by, routing_key, payload, occurred_at,
           attempts, why
    FROM dead_letters
    ORDER BY dead_letter
";

const WAITING: &str = "SELECT count(*) FROM deliveries";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteDeliveries {
    pool: SqlitePool,
    waiting: Arc<Notify>,
}

struct Waiting(Arc<Notify>);

#[async_trait]
impl Notifications for Waiting {
    async fn wait(&mut self) {
        self.0.notified().await;
    }
}

impl SqliteDeliveries {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            waiting: Arc::new(Notify::new()),
        }
    }
}

fn unreachable(failure: impl std::error::Error) -> DeliveryError {
    DeliveryError::Unreachable(failure.to_string())
}

fn unreadable(delivery: i64, why: impl std::fmt::Display) -> DeliveryError {
    DeliveryError::Unreadable {
        delivery,
        why: why.to_string(),
    }
}

fn uuid(delivery: i64, named: &str, written: &str) -> Result<Uuid, DeliveryError> {
    Uuid::parse_str(written).map_err(|why| unreadable(delivery, format!("{named}: {why}")))
}

fn message_in(
    row: &SqliteRow,
    delivery: i64,
) -> Result<(ListenerName, Message, i32), DeliveryError> {
    let listener: String = row.try_get("listener").map_err(unreachable)?;
    let id: String = row.try_get("message_id").map_err(unreachable)?;
    let conversation: String = row.try_get("conversation").map_err(unreachable)?;
    let caused_by: Option<String> = row.try_get("caused_by").map_err(unreachable)?;
    let routing: String = row.try_get("routing_key").map_err(unreachable)?;
    let payload: String = row.try_get("payload").map_err(unreachable)?;
    let occurred_at: String = row.try_get("occurred_at").map_err(unreachable)?;
    let attempts: i32 = row.try_get("attempts").map_err(unreachable)?;

    Ok((
        ListenerName::parse(&listener).map_err(|why| unreadable(delivery, why))?,
        Message {
            id: MessageId::of(uuid(delivery, "message_id", &id)?),
            routing: RoutingKey::parse(&routing).map_err(|why| unreadable(delivery, why))?,
            conversation: Conversation::begun_by(MessageId::of(uuid(
                delivery,
                "conversation",
                &conversation,
            )?)),
            caused_by: caused_by
                .map(|caused| uuid(delivery, "caused_by", &caused).map(MessageId::of))
                .transpose()?,
            occurred_at: text::read(&occurred_at)
                .ok_or_else(|| unreadable(delivery, format!("occurred_at: {occurred_at}")))?,
            payload: serde_json::from_str(&payload)
                .map_err(|why| unreadable(delivery, format!("payload: {why}")))?,
        },
        attempts,
    ))
}

fn written_uuid(id: &MessageId) -> String {
    id.as_uuid().hyphenated().to_string()
}

#[async_trait]
impl Deliveries for SqliteDeliveries {
    async fn enqueue(
        &self,
        listener: &ListenerName,
        message: &Message,
    ) -> Result<(), DeliveryError> {
        sqlx::query(ENQUEUE)
            .bind(listener.as_str())
            .bind(written_uuid(&message.id))
            .bind(written_uuid(&message.conversation.as_message_id()))
            .bind(message.caused_by.as_ref().map(written_uuid))
            .bind(message.routing.to_string())
            .bind(message.payload.to_string())
            .bind(text::written(message.occurred_at))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        self.waiting.notify_one();

        Ok(())
    }

    async fn claim_due(
        &self,
        now: OffsetDateTime,
        at_most: i64,
    ) -> Result<Vec<Queued>, DeliveryError> {
        let claimed = sqlx::query(CLAIM_DUE)
            .bind(text::written(now))
            .bind(text::written(now + CLAIM_FOR))
            .bind(at_most)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        let mut queued = claimed
            .iter()
            .map(|row| {
                let id: i64 = row.try_get("delivery").map_err(unreachable)?;
                let (listener, message, attempts) = message_in(row, id)?;

                Ok(Queued {
                    id,
                    listener,
                    message,
                    attempts,
                })
            })
            .collect::<Result<Vec<_>, DeliveryError>>()?;
        queued.sort_by_key(|delivery| delivery.id);

        Ok(queued)
    }

    async fn mark_as_handled(&self, delivery: i64) -> Result<(), DeliveryError> {
        sqlx::query(HANDLED)
            .bind(delivery)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn mark_as_refused(
        &self,
        delivery: i64,
        why: &str,
        again_at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        sqlx::query(REFUSED)
            .bind(delivery)
            .bind(text::written(again_at))
            .bind(why)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn give_up(&self, delivery: i64, why: &str) -> Result<(), DeliveryError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(BURY)
            .bind(delivery)
            .bind(why)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(HANDLED)
            .bind(delivery)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, DeliveryError> {
        let found = sqlx::query(DEAD_LETTERS)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let (listener, message, attempts) = message_in(row, 0)?;
                let why: String = row.try_get("why").map_err(unreachable)?;

                Ok(DeadLetter {
                    listener,
                    message,
                    attempts,
                    why,
                })
            })
            .collect()
    }

    async fn waiting(&self) -> Result<usize, DeliveryError> {
        let held: i64 = sqlx::query_scalar(WAITING)
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(held as usize)
    }

    async fn notifications(&self) -> Result<Box<dyn Notifications>, DeliveryError> {
        Ok(Box::new(Waiting(self.waiting.clone())))
    }
}
