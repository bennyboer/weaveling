use async_trait::async_trait;
use passages_core::{Passage, PassageId, PassageStore, PassageTitle, ProjectLink, StoreError};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Postgres, Row, Transaction};

const REMEMBER: &str = "INSERT INTO passages (passage, project, title) VALUES ($1, $2, $3)";

const KNOWN_AS: &str = "SELECT project, title FROM passages WHERE passage = $1";

const RETITLE: &str = "UPDATE passages SET title = $2 WHERE passage = $1";

const IN_PROJECT: &str = "
    SELECT passage
    FROM passages
    WHERE project = $1 AND passage > $2
    ORDER BY passage
    LIMIT $3
";

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

const HOLD: &str = "SELECT project FROM passages WHERE passage = $1 FOR UPDATE";

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

        let found: Option<String> = sqlx::query_scalar(HOLD)
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(unreachable)?;
        let Some(project) = found else {
            return Err(StoreError::NotFound(id));
        };

        let parts = self.parts(&mut transaction, id).await?;
        if parts.len() as i64 <= self.compact_after {
            return Ok(false);
        }

        let passage = grown(id, ProjectLink::from(project), &parts)?;
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

fn grown(id: PassageId, project: ProjectLink, parts: &[Vec<u8>]) -> Result<Passage, StoreError> {
    let passage = Passage::empty(id, project);

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
            .bind(passage.project().to_string())
            .bind(passage.title().as_str())
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

        let (project, title): (String, String) = sqlx::query_as(KNOWN_AS)
            .bind(id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;
        let title = PassageTitle::new(&title).map_err(|why| StoreError::Backend(Box::new(why)))?;

        Ok(grown(id, ProjectLink::from(project), &parts)?.titled(title))
    }

    async fn apply(&self, id: PassageId, update: &[u8]) -> Result<(), StoreError> {
        Passage::readable(update).map_err(|_| StoreError::Unusable(id))?;

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

    async fn retitle(&self, id: PassageId, title: &PassageTitle) -> Result<(), StoreError> {
        let changed = sqlx::query(RETITLE)
            .bind(id.to_string())
            .bind(title.as_str())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match changed.rows_affected() {
            0 => Err(StoreError::NotFound(id)),
            _ => Ok(()),
        }
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<PassageId>,
        at_most: usize,
    ) -> Result<Vec<PassageId>, StoreError> {
        let found: Vec<String> = sqlx::query_scalar(IN_PROJECT)
            .bind(project.to_string())
            .bind(after.map(|last| last.to_string()).unwrap_or_default())
            .bind(at_most as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|passage| {
                passage
                    .parse()
                    .map_err(|why| StoreError::Backend(Box::new(why)))
            })
            .collect()
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
