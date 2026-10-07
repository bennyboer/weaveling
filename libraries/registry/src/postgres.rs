use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};

use crate::{Registry, RegistryError};

const CLAIM: &str = "
    INSERT INTO claims (kind, key, id)
    VALUES ($1, $2, $3)
    ON CONFLICT (kind, key) DO UPDATE SET kind = EXCLUDED.kind
    RETURNING id
";

const LEDGER: &str = "_sqlx_migrations_claims";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresRegistry {
    pool: PgPool,
}

impl PostgresRegistry {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> RegistryError {
    RegistryError::Backend(Box::new(failure))
}

#[async_trait]
impl Registry for PostgresRegistry {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ledger_is_renamed_at_all() {
        assert_eq!(migrations().table_name, LEDGER);
    }

    #[test]
    fn the_ledger_is_never_renamed_to_the_one_a_feature_will_want() {
        assert_ne!(migrations().table_name, Migrator::DEFAULT.table_name);
        assert_ne!(
            migrations().table_name,
            "_sqlx_migrations_events",
            "a feature lays the event store and the registry down in one schema, so their ledgers \
             must not be the same table"
        );
    }
}
