use async_trait::async_trait;
use serde_json::Value;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::delivering::{CLAIM_FOR, DeadLetter, Deliveries, DeliveryError, Queued};
use crate::listening::{ListenerName, Notifications};
use crate::message::{Conversation, Message, MessageId};
use crate::routing::RoutingKey;

const LEDGER: &str = "_sqlx_migrations_deliveries";

const ENQUEUE: &str = "
    INSERT INTO deliveries
        (listener, message_id, conversation, caused_by, routing_key, payload, occurred_at, due_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
";

const CLAIM_DUE: &str = "
    WITH due AS (
        SELECT delivery
        FROM deliveries
        WHERE due_at <= $1 AND (claimed_until IS NULL OR claimed_until < $1)
        ORDER BY delivery
        LIMIT $3
        FOR UPDATE SKIP LOCKED
    )
    UPDATE deliveries
    SET claimed_until = $2
    WHERE delivery IN (SELECT delivery FROM due)
    RETURNING delivery, listener, message_id, conversation, caused_by, routing_key, payload,
              occurred_at, attempts
";

const HANDLED: &str = "DELETE FROM deliveries WHERE delivery = $1";

const REFUSED: &str = "
    UPDATE deliveries
    SET attempts = attempts + 1, due_at = $2, claimed_until = NULL, last_refusal = $3
    WHERE delivery = $1
";

const GIVE_UP: &str = "
    WITH gone AS (
        DELETE FROM deliveries
        WHERE delivery = $1
        RETURNING listener, message_id, conversation, caused_by, routing_key, payload,
                  occurred_at, attempts
    )
    INSERT INTO dead_letters
        (listener, message_id, conversation, caused_by, routing_key, payload, occurred_at,
         attempts, why)
    SELECT listener, message_id, conversation, caused_by, routing_key, payload, occurred_at,
           attempts + 1, $2
    FROM gone
";

const DEAD_LETTERS: &str = "
    SELECT listener, message_id, conversation, caused_by, routing_key, payload, occurred_at,
           attempts, why
    FROM dead_letters
    ORDER BY dead_letter
";

const WAITING: &str = "SELECT count(*) FROM deliveries";

const CHANNEL: &str = "deliveries_waiting_";

const NOTIFY: &str = "SELECT pg_notify(left('deliveries_waiting_' || current_schema(), 63), '')";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresDeliveries {
    pool: PgPool,
}

impl PostgresDeliveries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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

fn message_in(row: &PgRow, delivery: i64) -> Result<(ListenerName, Message, i32), DeliveryError> {
    let listener: String = row.try_get("listener").map_err(unreachable)?;
    let id: Uuid = row.try_get("message_id").map_err(unreachable)?;
    let conversation: Uuid = row.try_get("conversation").map_err(unreachable)?;
    let caused_by: Option<Uuid> = row.try_get("caused_by").map_err(unreachable)?;
    let routing: String = row.try_get("routing_key").map_err(unreachable)?;
    let payload: Value = row.try_get("payload").map_err(unreachable)?;
    let occurred_at: OffsetDateTime = row.try_get("occurred_at").map_err(unreachable)?;
    let attempts: i32 = row.try_get("attempts").map_err(unreachable)?;

    Ok((
        ListenerName::parse(&listener).map_err(|why| unreadable(delivery, why))?,
        Message {
            id: MessageId::of(id),
            routing: RoutingKey::parse(&routing).map_err(|why| unreadable(delivery, why))?,
            conversation: Conversation::begun_by(MessageId::of(conversation)),
            caused_by: caused_by.map(MessageId::of),
            occurred_at,
            payload,
        },
        attempts,
    ))
}

#[async_trait]
impl Deliveries for PostgresDeliveries {
    async fn enqueue(
        &self,
        listener: &ListenerName,
        message: &Message,
    ) -> Result<(), DeliveryError> {
        sqlx::query(ENQUEUE)
            .bind(listener.as_str())
            .bind(message.id.as_uuid())
            .bind(message.conversation.as_message_id().as_uuid())
            .bind(message.caused_by.map(|caused| caused.as_uuid()))
            .bind(message.routing.to_string())
            .bind(&message.payload)
            .bind(message.occurred_at)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        sqlx::query(NOTIFY)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn claim_due(
        &self,
        now: OffsetDateTime,
        at_most: i64,
    ) -> Result<Vec<Queued>, DeliveryError> {
        let claimed = sqlx::query(CLAIM_DUE)
            .bind(now)
            .bind(now + CLAIM_FOR)
            .bind(at_most)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        claimed
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
            .collect()
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
            .bind(again_at)
            .bind(why)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn give_up(&self, delivery: i64, why: &str) -> Result<(), DeliveryError> {
        sqlx::query(GIVE_UP)
            .bind(delivery)
            .bind(why)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
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
        let listening = crate::notifying::listening_to(&self.pool, CHANNEL)
            .await
            .map_err(unreachable)?;

        Ok(Box::new(listening))
    }
}
