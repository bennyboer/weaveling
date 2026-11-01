use std::sync::LazyLock;

use messaging::Message;
use tokio::sync::watch;
use transacting::Transaction;

use crate::outbox::{Origin, OutboxError};
use clock::text;

use crate::sqlite;

static ENQUEUED: LazyLock<watch::Sender<u64>> = LazyLock::new(|| watch::channel(0).0);

pub(crate) fn hearing_enqueued() -> watch::Receiver<u64> {
    ENQUEUED.subscribe()
}

const ENQUEUE: &str = "
    INSERT INTO outbox
        (aggregate, kind, version, message_id, conversation, caused_by,
         routing_key, payload, occurred_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
";

pub async fn enqueue(
    transaction: &mut Transaction,
    origin: Origin<'_>,
    message: &Message,
) -> Result<(), OutboxError> {
    sqlx::query(ENQUEUE)
        .bind(origin.aggregate)
        .bind(origin.kind)
        .bind(sqlite::as_integer(origin.version))
        .bind(message.id.as_uuid().hyphenated().to_string())
        .bind(
            message
                .conversation
                .as_message_id()
                .as_uuid()
                .hyphenated()
                .to_string(),
        )
        .bind(
            message
                .caused_by
                .map(|caused| caused.as_uuid().hyphenated().to_string()),
        )
        .bind(message.routing.to_string())
        .bind(message.payload.to_string())
        .bind(text::written(message.occurred_at))
        .execute(transacting::sqlite::connection(transaction)?)
        .await
        .map_err(|failure| OutboxError::Unreachable(failure.to_string()))?;
    transaction.after_commit(async { ENQUEUED.send_modify(|rung| *rung = rung.wrapping_add(1)) });

    Ok(())
}
