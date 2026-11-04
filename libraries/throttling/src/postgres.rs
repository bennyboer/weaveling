use std::error::Error;

use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::{Admission, Rule, Throttles, ThrottlesError, Tries};

const TRIES: &str = "SELECT counted, last FROM throttles WHERE guarded = $1 AND about = $2";

const FIRST: &str = "
    INSERT INTO throttles (guarded, about, counted, last)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (guarded, about) DO NOTHING
";

const NEXT: &str = "
    UPDATE throttles
    SET counted = $3, last = $4
    WHERE guarded = $1 AND about = $2 AND counted = $5 AND last = $6
";

const CLEAR: &str = "DELETE FROM throttles WHERE guarded = $1 AND about = $2";

const LEDGER: &str = "_sqlx_migrations_throttles";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresThrottles {
    pool: PgPool,
}

impl PostgresThrottles {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> ThrottlesError {
    ThrottlesError::Backend(Box::new(failure))
}

fn to_tries(row: &PgRow) -> Result<Tries, ThrottlesError> {
    let counted: i32 = row.try_get("counted").map_err(unreachable)?;
    let last: OffsetDateTime = row.try_get("last").map_err(unreachable)?;

    Ok(Tries {
        counted: u32::try_from(counted).map_err(unreachable)?,
        last,
    })
}

#[async_trait]
impl Throttles for PostgresThrottles {
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
            let counted = i32::try_from(next.counted).map_err(unreachable)?;
            let counted_before = before
                .map(|before| i32::try_from(before.counted))
                .transpose()
                .map_err(unreachable)?;
            let written = transacting::postgres::retried(|| {
                match (before, counted_before) {
                    (Some(before), Some(counted_before)) => sqlx::query(NEXT)
                        .bind(guarded)
                        .bind(about)
                        .bind(counted)
                        .bind(next.last)
                        .bind(counted_before)
                        .bind(before.last),
                    _ => sqlx::query(FIRST)
                        .bind(guarded)
                        .bind(about)
                        .bind(counted)
                        .bind(next.last),
                }
                .execute(&self.pool)
            })
            .await
            .map_err(unreachable)?;

            if written.rows_affected() == 1 {
                return Ok(Admission::Admitted);
            }
        }
    }

    async fn clear(&self, guarded: &str, about: &str) -> Result<(), ThrottlesError> {
        transacting::postgres::retried(|| {
            sqlx::query(CLEAR)
                .bind(guarded)
                .bind(about)
                .execute(&self.pool)
        })
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
        // expect: the ledger is the throttles' own, not the default one
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(
            migrations().table_name,
            Migrator::DEFAULT.table_name,
            "every feature lays the throttles down beside a 0001 of its own"
        );
    }
}
