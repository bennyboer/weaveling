use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::postgres::PostgresEventStore;
use crate::store::StoreError;
use crate::version::Version;

const SINCE: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = $1 AND kind = $2 AND version >= $3
    ORDER BY version
";

const BETWEEN: &str = "
    SELECT version, body, agent, occurred_at, is_snapshot
    FROM events
    WHERE aggregate = $1 AND kind = $2 AND version >= $3 AND version <= $4
    ORDER BY version
";

impl<E> PostgresEventStore<E> {
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
}
