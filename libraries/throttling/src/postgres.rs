use std::error::Error;

use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::{Throttles, ThrottlesError, Tries};

const TRIES: &str = "SELECT counted, last FROM throttles WHERE guarded = $1 AND about = $2";

const COUNT: &str = "
    INSERT INTO throttles (guarded, about, counted, last)
    VALUES ($1, $2, 1, $3)
    ON CONFLICT (guarded, about) DO UPDATE
    SET counted = throttles.counted + 1,
        last    = EXCLUDED.last
    RETURNING counted, last
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

    async fn count(
        &self,
        guarded: &str,
        about: &str,
        at: OffsetDateTime,
    ) -> Result<Tries, ThrottlesError> {
        let counted = sqlx::query(COUNT)
            .bind(guarded)
            .bind(about)
            .bind(at)
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;

        to_tries(&counted)
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
