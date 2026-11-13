use assets_core::{AssetId, Contents, ContentsError, Piece};
use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::migrate::Migrator;

use crate::unreachable;

const KEEP: &str = "
    INSERT INTO asset_contents (asset, piece, bytes) VALUES ($1, $2, $3)
    ON CONFLICT (asset, piece) DO UPDATE SET bytes = excluded.bytes
";

const READ: &str = "SELECT bytes FROM asset_contents WHERE asset = $1 AND piece = $2";

const FORGET: &str = "DELETE FROM asset_contents WHERE asset = $1";

const STORED: &str = "
    SELECT DISTINCT asset FROM asset_contents
    WHERE asset > $1
    ORDER BY asset
    LIMIT $2
";

const LEDGER: &str = "_sqlx_migrations_asset_contents";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresContents {
    pool: PgPool,
}

impl PostgresContents {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Contents for PostgresContents {
    async fn keep(&self, asset: &AssetId, piece: Piece, bytes: &[u8]) -> Result<(), ContentsError> {
        sqlx::query(KEEP)
            .bind(asset.to_string())
            .bind(piece.name())
            .bind(bytes)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn read(&self, asset: &AssetId, piece: Piece) -> Result<Option<Vec<u8>>, ContentsError> {
        sqlx::query_scalar(READ)
            .bind(asset.to_string())
            .bind(piece.name())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)
    }

    async fn forget(&self, asset: &AssetId) -> Result<(), ContentsError> {
        sqlx::query(FORGET)
            .bind(asset.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn stored(
        &self,
        after: Option<&AssetId>,
        at_most: u64,
    ) -> Result<Vec<AssetId>, ContentsError> {
        let stored: Vec<String> = sqlx::query_scalar(STORED)
            .bind(after.map(ToString::to_string).unwrap_or_default())
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        stored
            .iter()
            .map(|asset| asset.parse().map_err(unreachable))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ledger_is_the_contents_own() {
        // expect: the ledger is neither the default one nor another's
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(
            migrations().table_name,
            Migrator::DEFAULT.table_name,
            "the feature lays its other tables down in the same schema, so their ledgers must not be the same table"
        );
    }
}
