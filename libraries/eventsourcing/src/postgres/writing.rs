use sqlx::{Postgres, Transaction};

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::{Event, Recorded};
use crate::postgres::{PostgresEventStore, agent, as_bigint};
use crate::store::StoreError;
use crate::version::Version;

const HEAD: &str = "
    SELECT COALESCE(MAX(version), 0)
    FROM events
    WHERE aggregate = $1 AND kind = $2
";

const WRITE: &str = "
    INSERT INTO events
        (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
";

impl<E> PostgresEventStore<E>
where
    E: Event,
{
    pub(super) async fn write(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        events: &[Recorded<E>],
    ) -> Result<(), StoreError> {
        if events.is_empty() {
            return Ok(());
        }

        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|failure| self.backend_error(aggregate, kind, failure.to_string()))?;

        let head: i64 = sqlx::query_scalar(HEAD)
            .bind(aggregate.as_str())
            .bind(kind.as_str())
            .fetch_one(&mut *transaction)
            .await
            .map_err(|failure| self.backend_error(aggregate, kind, failure.to_string()))?;

        if head != as_bigint(expected) {
            return Err(self.outdated(aggregate, kind, expected));
        }

        for happened in events {
            self.insert(&mut transaction, aggregate, kind, expected, happened)
                .await?;
            self.announce(&mut transaction, aggregate, kind, happened)
                .await?;
        }

        transaction
            .commit()
            .await
            .map_err(|failure| self.backend_error(aggregate, kind, failure.to_string()))
    }

    async fn insert(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        happened: &Recorded<E>,
    ) -> Result<(), StoreError> {
        sqlx::query(WRITE)
            .bind(aggregate.as_str())
            .bind(kind.as_str())
            .bind(as_bigint(happened.metadata.version))
            .bind(happened.event.name().as_str())
            .bind((self.codec.body)(&happened.event))
            .bind(as_bigint(happened.event.version()))
            .bind(agent::encode(&happened.metadata.agent))
            .bind(happened.metadata.occurred_at)
            .bind(happened.metadata.is_snapshot)
            .execute(&mut **transaction)
            .await
            .map_err(|failure| match is_unique_violation(&failure) {
                true => self.outdated(aggregate, kind, expected),
                false => self.backend_error(aggregate, kind, failure.to_string()),
            })?;

        Ok(())
    }
}

fn is_unique_violation(failure: &sqlx::Error) -> bool {
    matches!(failure, sqlx::Error::Database(reason) if reason.is_unique_violation())
}
