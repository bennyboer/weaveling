use serde_json::Value;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Row, Sqlite};

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::metadata::EventMetadata;
use crate::sqlite::{SqliteEventStore, agent, as_integer, instant};
use crate::store::StoreError;
use crate::version::Version;

const SINCE: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = ?1 AND kind = ?2 AND version >= ?3
    ORDER BY version
";

const BETWEEN: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = ?1 AND kind = ?2 AND version >= ?3 AND version <= ?4
    ORDER BY version
";

const NEWEST: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = ?1 AND kind = ?2 AND is_snapshot
    ORDER BY version DESC
    LIMIT 1
";

const NEWEST_BY: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = ?1 AND kind = ?2 AND is_snapshot AND version <= ?3
    ORDER BY version DESC
    LIMIT 1
";

const DISCARD: &str = "DELETE FROM events WHERE aggregate = ?1 AND kind = ?2 AND version <= ?3";

impl<E> SqliteEventStore<E> {
    pub(super) async fn since(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.all(SINCE, aggregate, kind, &[from]).await
    }

    pub(super) async fn between(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.all(BETWEEN, aggregate, kind, &[from, through]).await
    }

    pub(super) async fn newest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.one(NEWEST, aggregate, kind, &[]).await
    }

    pub(super) async fn snapshot_up_to(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.one(NEWEST_BY, aggregate, kind, &[version]).await
    }

    pub(super) async fn discard_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        query_for(DISCARD, aggregate, kind, &[through])
            .execute(&self.pool)
            .await
            .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))?;

        Ok(())
    }

    async fn all(
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

    async fn one(
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

    fn recorded(
        &self,
        row: &SqliteRow,
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
        let body: String = column(row, "body", aggregate, kind)?;
        let agent: String = column(row, "agent", aggregate, kind)?;
        let occurred_at: String = column(row, "occurred_at", aggregate, kind)?;
        let is_snapshot: bool = column(row, "is_snapshot", aggregate, kind)?;

        let unreadable = |what: &str| {
            StoreError::backend(
                aggregate,
                kind,
                format!("version {version} was written with {what} nothing can read"),
            )
        };
        let body: Value = serde_json::from_str(&body).map_err(|_| unreadable("a body"))?;
        let event = (self.codec.event)(body).ok_or_else(|| unreadable("a shape"))?;
        let occurred_at = instant::read(&occurred_at).ok_or_else(|| unreadable("a time"))?;

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
}

fn query_for<'q>(
    statement: &'static str,
    aggregate: &'q AggregateId,
    kind: AggregateType,
    versions: &[Version],
) -> sqlx::query::Query<'q, Sqlite, SqliteArguments> {
    let mut query = sqlx::query(statement)
        .bind(aggregate.as_str())
        .bind(kind.as_str());

    for version in versions {
        query = query.bind(as_integer(*version));
    }

    query
}

fn column<'row, T>(
    row: &'row SqliteRow,
    column: &'static str,
    aggregate: &AggregateId,
    kind: AggregateType,
) -> Result<T, StoreError>
where
    T: sqlx::Decode<'row, Sqlite> + sqlx::Type<Sqlite>,
{
    row.try_get(column).map_err(|failure| {
        StoreError::backend(
            aggregate,
            kind,
            format!("{column} could not be read: {failure}"),
        )
    })
}
