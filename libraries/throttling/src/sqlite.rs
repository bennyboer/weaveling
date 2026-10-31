use std::error::Error;

use async_trait::async_trait;
use clock::text;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

use crate::{Admission, Rule, Throttles, ThrottlesError, Tries};

const TRIES: &str = "SELECT counted, last FROM throttles WHERE guarded = ?1 AND about = ?2";

const FIRST: &str = "
    INSERT INTO throttles (guarded, about, counted, last)
    VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT (guarded, about) DO NOTHING
";

const NEXT: &str = "
    UPDATE throttles
    SET counted = ?3, last = ?4
    WHERE guarded = ?1 AND about = ?2 AND counted = ?5 AND last = ?6
";

const CLEAR: &str = "DELETE FROM throttles WHERE guarded = ?1 AND about = ?2";

const LEDGER: &str = "_sqlx_migrations_throttles";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteThrottles {
    pool: SqlitePool,
}

impl SqliteThrottles {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> ThrottlesError {
    ThrottlesError::Backend(Box::new(failure))
}

fn to_tries(row: &SqliteRow) -> Result<Tries, ThrottlesError> {
    let counted: i64 = row.try_get("counted").map_err(unreachable)?;
    let last: String = row.try_get("last").map_err(unreachable)?;
    let last = text::read(&last).ok_or_else(|| {
        ThrottlesError::Backend(format!("last holds {last:?}, which is no time").into())
    })?;

    Ok(Tries {
        counted: u32::try_from(counted).map_err(unreachable)?,
        last,
    })
}

#[async_trait]
impl Throttles for SqliteThrottles {
    async fn tries(&self, guarded: &str, about: &str) -> Result<Option<Tries>, ThrottlesError> {
        sqlx::query(TRIES)
            .bind(guarded)
            .bind(about)
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(to_tries)
            .transpose()
    }

    async fn admit(
        &self,
        guarded: &str,
        about: &str,
        rule: Rule,
        at: OffsetDateTime,
    ) -> Result<Admission, ThrottlesError> {
        loop {
            let before = self.tries(guarded, about).await?;
            let next = match Tries::admitted(before, rule, at) {
                Ok(next) => next,
                Err(wait) => return Ok(Admission::Wait(wait)),
            };
            let counted = i64::from(next.counted);
            let written = match before {
                None => sqlx::query(FIRST)
                    .bind(guarded)
                    .bind(about)
                    .bind(counted)
                    .bind(text::written(next.last)),
                Some(before) => sqlx::query(NEXT)
                    .bind(guarded)
                    .bind(about)
                    .bind(counted)
                    .bind(text::written(next.last))
                    .bind(i64::from(before.counted))
                    .bind(text::written(before.last)),
            }
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

            if written.rows_affected() == 1 {
                return Ok(Admission::Admitted);
            }
        }
    }

    async fn clear(&self, guarded: &str, about: &str) -> Result<(), ThrottlesError> {
        sqlx::query(CLEAR)
            .bind(guarded)
            .bind(about)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[test]
    fn the_throttles_keep_a_ledger_of_their_own() {
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(
            migrations().table_name,
            Migrator::DEFAULT.table_name,
            "every feature lays the throttles down beside a 0001 of its own"
        );
    }
}
