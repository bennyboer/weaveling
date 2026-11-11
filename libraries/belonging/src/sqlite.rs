use async_trait::async_trait;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use transacting::Transaction;

use crate::{Belongings, BelongingsError};

const REMEMBER: &str = "
    INSERT INTO belongings (kind, id, owner_kind, owner)
    VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT (kind, id, owner_kind) DO UPDATE SET owner = excluded.owner
";

const FORGET: &str = "DELETE FROM belongings WHERE kind = ?1 AND id = ?2";

const OWNER_OF: &str =
    "SELECT owner FROM belongings WHERE kind = ?1 AND id = ?2 AND owner_kind = ?3";

const LEDGER: &str = "_sqlx_migrations_belongings";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteBelongings {
    pool: SqlitePool,
}

impl SqliteBelongings {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> BelongingsError {
    BelongingsError::Backend(Box::new(failure))
}

#[async_trait]
impl Belongings for SqliteBelongings {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        owner_kind: &str,
        owner: &str,
    ) -> Result<(), BelongingsError> {
        sqlx::query(REMEMBER)
            .bind(kind)
            .bind(id)
            .bind(owner_kind)
            .bind(owner)
            .execute(transacting::sqlite::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
    ) -> Result<(), BelongingsError> {
        sqlx::query(FORGET)
            .bind(kind)
            .bind(id)
            .execute(transacting::sqlite::connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn owner_of(
        &self,
        kind: &str,
        id: &str,
        owner_kind: &str,
    ) -> Result<Option<String>, BelongingsError> {
        sqlx::query_scalar(OWNER_OF)
            .bind(kind)
            .bind(id)
            .bind(owner_kind)
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ledger_is_the_belongings_own() {
        // expect: the ledger is neither the default one nor another library's
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(
            migrations().table_name,
            Migrator::DEFAULT.table_name,
            "a feature lays its own tables and these down in one schema, so their ledgers must not \
             be the same table"
        );
    }
}
