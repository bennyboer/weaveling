use messaging::Message;
use sqlx::{Sqlite, Transaction};

use crate::outbox::Origin;
use crate::sqlite::{as_integer, instant};

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
        .bind(as_integer(origin.version))
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
        .bind(instant::stored(message.occurred_at))
        .execute(&mut **transaction)
        .await?;

    Ok(())
}
