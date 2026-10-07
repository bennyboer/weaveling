use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row};
use time::OffsetDateTime;

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::metadata::EventMetadata;
use crate::postgres::{PostgresEventStore, agent, as_bigint};
use crate::store::StoreError;
use crate::version::Version;

impl<E> PostgresEventStore<E> {
    pub(super) fn recorded(
        &self,
        row: &PgRow,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Recorded<E>, StoreError> {
        let stored: i64 = column(row, "version", aggregate, kind)?;
        let version = u64::try_from(stored).map_err(|_| {
            StoreError::backend(
                aggregate,
                kind,
                format!("{stored} was written where a version belongs"),
            )
        })?;
        let body: Value = column(row, "body", aggregate, kind)?;
        let agent: String = column(row, "agent", aggregate, kind)?;
        let occurred_at: OffsetDateTime = column(row, "occurred_at", aggregate, kind)?;
        let is_snapshot: bool = column(row, "is_snapshot", aggregate, kind)?;

        let event = (self.codec.event)(body).ok_or_else(|| {
            StoreError::backend(
                aggregate,
                kind,
                format!("version {version} was written in a shape nothing can read"),
            )
        })?;

        Ok(Recorded {
            event,
            metadata: EventMetadata {
                aggregate: aggregate.clone(),
                kind,
                version: Version::of(version),
                agent: agent::decode(&agent),
                occurred_at,
                is_snapshot,
            },
        })
    }

    pub(super) async fn all(
        &self,
        statement: &'static str,
        aggregate: &AggregateId,
        kind: AggregateType,
        versions: &[Version],
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        let found = query_for(statement, aggregate, kind, versions)
            .fetch_all(&self.pool)
            .await
            .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))?;

        found
            .iter()
            .map(|row| self.recorded(row, aggregate, kind))
            .collect()
    }

    pub(super) async fn one(
        &self,
        statement: &'static str,
        aggregate: &AggregateId,
        kind: AggregateType,
        versions: &[Version],
    ) -> Result<Option<Recorded<E>>, StoreError> {
        let found = query_for(statement, aggregate, kind, versions)
            .fetch_optional(&self.pool)
            .await
            .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))?;

        found
            .map(|row| self.recorded(&row, aggregate, kind))
            .transpose()
    }
}

fn query_for<'q>(
    statement: &'static str,
    aggregate: &'q AggregateId,
    kind: AggregateType,
    versions: &[Version],
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    let mut query = sqlx::query(statement)
        .bind(aggregate.as_str())
        .bind(kind.as_str());

    for version in versions {
        query = query.bind(as_bigint(*version));
    }

    query
}

fn column<'row, T>(
    row: &'row PgRow,
    column: &'static str,
    aggregate: &AggregateId,
    kind: AggregateType,
) -> Result<T, StoreError>
where
    T: sqlx::Decode<'row, Postgres> + sqlx::Type<Postgres>,
{
    row.try_get(column).map_err(|failure| {
        StoreError::backend(
            aggregate,
            kind,
            format!("{column} could not be read: {failure}"),
        )
    })
}
