use messaging::Message;
use transacting::Transaction;

use crate::outbox::{Origin, OutboxError};
use crate::postgres;

const NOTIFY: &str = "SELECT pg_notify(left('outbox_waiting_' || current_schema(), 63), '')";

const ENQUEUE: &str = "
    INSERT INTO outbox
        (aggregate, kind, version, message_id, conversation, caused_by,
         routing_key, payload, occurred_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
";

pub async fn enqueue(
    transaction: &mut Transaction,
    origin: Origin<'_>,
    message: &Message,
) -> Result<(), OutboxError> {
    let connection = transacting::postgres::connection(transaction)?;
    sqlx::query(ENQUEUE)
        .bind(origin.aggregate)
        .bind(origin.kind)
        .bind(origin.version.map(postgres::as_bigint))
        .bind(message.id.as_uuid())
        .bind(message.conversation.as_message_id().as_uuid())
        .bind(message.caused_by.map(|caused| caused.as_uuid()))
        .bind(message.routing.to_string())
        .bind(&message.payload)
        .bind(message.occurred_at)
        .execute(&mut *connection)
        .await
        .map_err(unreachable)?;

    sqlx::query(NOTIFY)
        .execute(&mut *connection)
        .await
        .map_err(unreachable)?;

    Ok(())
}

fn unreachable(failure: sqlx::Error) -> OutboxError {
    OutboxError::Unreachable(failure.to_string())
}
