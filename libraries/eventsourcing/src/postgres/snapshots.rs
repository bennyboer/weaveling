use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::postgres::{PostgresEventStore, as_bigint};
use crate::store::StoreError;
use crate::version::Version;

const NEWEST: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = $1 AND kind = $2 AND is_snapshot
    ORDER BY version DESC
    LIMIT 1
";

const NEWEST_BY: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = $1 AND kind = $2 AND is_snapshot AND version <= $3
    ORDER BY version DESC
    LIMIT 1
";

const DISCARD: &str = "DELETE FROM events WHERE aggregate = $1 AND kind = $2 AND version <= $3";

impl<E> PostgresEventStore<E> {
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
        sqlx::query(DISCARD)
            .bind(aggregate.as_str())
            .bind(kind.as_str())
            .bind(as_bigint(through))
            .execute(&self.pool)
            .await
            .map_err(|failure| self.backend_error(aggregate, kind, failure.to_string()))?;

        Ok(())
    }
}
