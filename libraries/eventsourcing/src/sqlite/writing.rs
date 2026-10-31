use clock::text;
use outbox::Origin;
use transacting::Transaction;

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::{Event, Recorded};
use crate::sqlite::{SqliteEventStore, agent};
use crate::store::StoreError;
use crate::version::Version;

const HEAD: &str = "
    SELECT COALESCE(MAX(version), 0)
    FROM events
    WHERE aggregate = ?1 AND kind = ?2
";

const WRITE: &str = "
    INSERT INTO events
        (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot, context)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
";

impl<E> SqliteEventStore<E>
where
    E: Event,
{
    pub(super) async fn write(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        events: &[Recorded<E>],
    ) -> Result<(), StoreError> {
        if events.is_empty() {
            return Ok(());
        }

        let head: i64 = sqlx::query_scalar(HEAD)
            .bind(aggregate.as_str())
            .bind(kind.as_str())
            .fetch_one(
                transacting::sqlite::connection(transaction)
                    .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))?,
            )
            .await
            .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))?;

        if head != crate::sqlite::as_integer(expected) {
            return Err(StoreError::outdated(aggregate, kind, expected));
        }

        for happened in events {
            self.insert(transaction, aggregate, kind, expected, happened)
                .await?;
            self.announce(transaction, aggregate, kind, happened)
                .await?;
        }

        Ok(())
    }

    async fn insert(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        happened: &Recorded<E>,
    ) -> Result<(), StoreError> {
        sqlx::query(WRITE)
            .bind(aggregate.as_str())
            .bind(kind.as_str())
            .bind(crate::sqlite::as_integer(happened.metadata.version))
            .bind(happened.event.name().as_str())
            .bind((self.codec.body)(&happened.event).to_string())
            .bind(crate::sqlite::as_integer(happened.event.version()))
            .bind(agent::encode(&happened.metadata.agent))
            .bind(text::written(happened.metadata.occurred_at))
            .bind(happened.metadata.is_snapshot)
            .bind(happened.metadata.context.written().to_string())
            .execute(
                transacting::sqlite::connection(transaction)
                    .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))?,
            )
            .await
            .map_err(|failure| match is_unique_violation(&failure) {
                true => StoreError::outdated(aggregate, kind, expected),
                false => StoreError::backend(aggregate, kind, failure.to_string()),
            })?;

        Ok(())
    }

    async fn announce(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        happened: &Recorded<E>,
    ) -> Result<(), StoreError> {
        let Some(message) = (self.message_for)(happened) else {
            return Ok(());
        };

        let origin = Origin {
            aggregate: aggregate.as_str(),
            kind: kind.as_str(),
            version: happened.metadata.version.count(),
        };

        outbox::sqlite::enqueue(transaction, origin, &message)
            .await
            .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))
    }
}

fn is_unique_violation(failure: &sqlx::Error) -> bool {
    matches!(failure, sqlx::Error::Database(reason) if reason.is_unique_violation())
}
