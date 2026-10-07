use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};

use crate::{Registry, RegistryError};

const CLAIM: &str = "
    INSERT INTO claims (kind, key, id)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (kind, key) DO UPDATE SET kind = excluded.kind
    RETURNING id
";

const HOLDER: &str = "SELECT id FROM claims WHERE kind = ?1 AND key = ?2";

const LEDGER: &str = "_sqlx_migrations_claims";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteRegistry {
    pool: SqlitePool,
}

impl SqliteRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> RegistryError {
    RegistryError::Backend(Box::new(failure))
}

#[async_trait]
impl Registry for SqliteRegistry {
    async fn claim(&self, kind: &str, key: &str, id: &str) -> Result<String, RegistryError> {
        let held = sqlx::query(CLAIM)
            .bind(kind)
            .bind(key)
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)?;

        held.try_get("id").map_err(unreachable)
    }

    async fn holder(&self, kind: &str, key: &str) -> Result<Option<String>, RegistryError> {
        sqlx::query_scalar(HOLDER)
            .bind(kind)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)
    }
}
