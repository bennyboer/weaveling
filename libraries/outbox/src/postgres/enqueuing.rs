use messaging::Message;
use sqlx::{Postgres, Transaction};

use crate::outbox::Origin;
use crate::postgres::as_bigint;

const NOTIFY: &str = "SELECT pg_notify(left('outbox_waiting_' || current_schema(), 63), '')";

const ENQUEUE: &str = "
    INSERT INTO outbox
        (aggregate, kind, version, message_id, conversation, caused_by,
         routing_key, payload, occurred_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
";

pub async fn enqueue(
    transaction: &mut Transaction<'_, Postgres>,
    origin: Origin<'_>,
    message: &Message,
) -> Result<(), sqlx::Error> {
    sqlx::query(ENQUEUE)
        .bind(origin.aggregate)
        .bind(origin.kind)
        .bind(as_bigint(origin.version))
        .bind(message.id.as_uuid())
        .bind(message.conversation.as_message_id().as_uuid())
        .bind(message.caused_by.map(|caused| caused.as_uuid()))
        .bind(message.routing.to_string())
        .bind(&message.payload)
        .bind(message.occurred_at)
        .execute(&mut **transaction)
        .await?;

    sqlx::query(NOTIFY).execute(&mut **transaction).await?;

    Ok(())
}
