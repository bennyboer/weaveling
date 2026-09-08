use async_trait::async_trait;
use passages_core::{Passage, PassageId, PassageStore, StoreError};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Postgres, Row, Transaction};

const REMEMBER: &str = "INSERT INTO passages (passage) VALUES ($1)";

const WRITE_SNAPSHOT: &str = "
    INSERT INTO passage_updates (passage, is_snapshot, bytes)
    VALUES ($1, TRUE, $2)
    RETURNING seq
";

const WRITE_UPDATE: &str = "
    INSERT INTO passage_updates (passage, is_snapshot, bytes)
    VALUES ($1, FALSE, $2)
    RETURNING (
        SELECT count(*)
        FROM passage_updates behind
        WHERE behind.passage = $1 AND behind.seq > COALESCE((
            SELECT max(newest.seq)
            FROM passage_updates newest
            WHERE newest.passage = $1 AND newest.is_snapshot
        ), 0)
    ) AS tail
";

const READ_SINCE_SNAPSHOT: &str = "
    SELECT bytes
    FROM passage_updates
    WHERE passage = $1 AND seq >= COALESCE((
        SELECT max(seq)
        FROM passage_updates
        WHERE passage = $1 AND is_snapshot
    ), 0)
    ORDER BY seq
";

const HOLD: &str = "SELECT passage FROM passages WHERE passage = $1 FOR UPDATE";

const FORGET_BEFORE: &str = "DELETE FROM passage_updates WHERE passage = $1 AND seq < $2";

const FORGET: &str = "DELETE FROM passages WHERE passage = $1";

pub const COMPACT_AFTER: i64 = 64;

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations")
}

pub struct PostgresPassageStore {
    pool: PgPool,
    compact_after: i64,
}

impl PostgresPassageStore {
    pub fn new(pool: PgPool) -> Self {
        Self::compacting_after(pool, COMPACT_AFTER)
    }

    pub fn compacting_after(pool: PgPool, updates: i64) -> Self {
        Self {
            pool,
            compact_after: updates,
        }
    }

    pub async fn compact(&self, id: PassageId) -> Result<bool, StoreError> {
        let mut transaction = self.begin().await?;

        let held = sqlx::query(HOLD)
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(unreachable)?;
        if held.is_none() {
            return Err(StoreError::NotFound(id));
        }

        let parts = self.parts(&mut transaction, id).await?;
        if parts.len() as i64 <= self.compact_after {
            return Ok(false);
        }

        let passage = grown(id, &parts)?;
        let collapsed: i64 = sqlx::query_scalar(WRITE_SNAPSHOT)
            .bind(id.to_string())
            .bind(passage.everything())
            .fetch_one(&mut *transaction)
            .await
            .map_err(unreachable)?;

        sqlx::query(FORGET_BEFORE)
            .bind(id.to_string())
            .bind(collapsed)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)?;

        Ok(true)
    }

    async fn begin(&self) -> Result<Transaction<'_, Postgres>, StoreError> {
        self.pool.begin().await.map_err(unreachable)
    }

    async fn parts(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        id: PassageId,
    ) -> Result<Vec<Vec<u8>>, StoreError> {
        let found = sqlx::query(READ_SINCE_SNAPSHOT)
            .bind(id.to_string())
            .fetch_all(&mut **transaction)
            .await
            .map_err(unreachable)?;

        Ok(found
            .iter()
            .map(|row| row.get::<Vec<u8>, _>("bytes"))
            .collect())
    }
}

fn grown(id: PassageId, parts: &[Vec<u8>]) -> Result<Passage, StoreError> {
    let passage = Passage::empty(id);

    for part in parts {
        passage
            .apply(part)
            .map_err(|reason| StoreError::Backend(Box::new(reason)))?;
    }

    Ok(passage)
}

fn unreachable(failure: sqlx::Error) -> StoreError {
    StoreError::Backend(Box::new(failure))
}

fn is_taken(failure: &sqlx::Error) -> bool {
    matches!(failure, sqlx::Error::Database(reason) if reason.is_unique_violation())
}

fn is_unknown(failure: &sqlx::Error) -> bool {
    matches!(failure, sqlx::Error::Database(reason) if reason.is_foreign_key_violation())
}

#[async_trait]
impl PassageStore for PostgresPassageStore {
    async fn create(&self, passage: &Passage) -> Result<(), StoreError> {
        let id = passage.id();
        let mut transaction = self.begin().await?;

        sqlx::query(REMEMBER)
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(|failure| match is_taken(&failure) {
                true => StoreError::Conflict(id),
                false => unreachable(failure),
            })?;

        sqlx::query(WRITE_SNAPSHOT)
            .bind(id.to_string())
            .bind(passage.everything())
            .fetch_one(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn load(&self, id: PassageId) -> Result<Passage, StoreError> {
        let found = sqlx::query(READ_SINCE_SNAPSHOT)
            .bind(id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        if found.is_empty() {
            return Err(StoreError::NotFound(id));
        }

        let parts: Vec<Vec<u8>> = found
            .iter()
            .map(|row| row.get::<Vec<u8>, _>("bytes"))
            .collect();

        grown(id, &parts)
    }

    async fn apply(&self, id: PassageId, update: &[u8]) -> Result<(), StoreError> {
        Passage::empty(id)
            .apply(update)
            .map_err(|_| StoreError::Unusable(id))?;

        let tail: i64 = sqlx::query_scalar(WRITE_UPDATE)
            .bind(id.to_string())
            .bind(update)
            .fetch_one(&self.pool)
            .await
            .map_err(|failure| match is_unknown(&failure) {
                true => StoreError::NotFound(id),
                false => unreachable(failure),
            })?;

        if tail + 1 > self.compact_after {
            self.compact(id).await?;
        }

        Ok(())
    }

    async fn delete(&self, id: PassageId) -> Result<(), StoreError> {
        let gone = sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match gone.rows_affected() {
            0 => Err(StoreError::NotFound(id)),
            _ => Ok(()),
        }
    }
}
