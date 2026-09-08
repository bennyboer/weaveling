use messaging::Message;
use sqlx::{Postgres, Transaction};

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::postgres::{PostgresEventStore, as_bigint};
use crate::store::StoreError;

const ANNOUNCE: &str = "
    INSERT INTO outbox
        (aggregate, kind, version, message_id, conversation, caused_by,
         routing_key, payload, occurred_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
";

pub type MessageMapping<E> = fn(&Recorded<E>) -> Option<Message>;

impl<E> PostgresEventStore<E> {
    pub(super) async fn announce(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        aggregate: &AggregateId,
        kind: AggregateType,
        happened: &Recorded<E>,
    ) -> Result<(), StoreError> {
        let Some(message) = (self.message_for)(happened) else {
            return Ok(());
        };

        sqlx::query(ANNOUNCE)
            .bind(aggregate.as_str())
            .bind(kind.as_str())
            .bind(as_bigint(happened.metadata.version))
            .bind(message.id.as_uuid())
            .bind(message.conversation.as_message_id().as_uuid())
            .bind(message.caused_by.map(|caused| caused.as_uuid()))
            .bind(message.routing.to_string())
            .bind(&message.payload)
            .bind(message.occurred_at)
            .execute(&mut **transaction)
            .await
            .map_err(|failure| self.backend_error(aggregate, kind, failure.to_string()))?;

        Ok(())
    }
}
