use std::sync::LazyLock;

use messaging::Message;
use sqlx::{Sqlite, Transaction};
use tokio::sync::watch;

use crate::outbox::Origin;
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
    transaction: &mut Transaction<'_, Sqlite>,
    origin: Origin<'_>,
    message: &Message,
) -> Result<(), sqlx::Error> {
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
        .execute(&mut **transaction)
        .await?;
    ENQUEUED.send_modify(|rung| *rung = rung.wrapping_add(1));

    Ok(())
}
