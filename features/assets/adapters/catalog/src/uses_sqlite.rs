use std::collections::BTreeSet;

use assets_core::{AssetId, AssetUse, AssetUses, CatalogError, Holder, Purpose};
use async_trait::async_trait;
use eventsourcing::Version;
use sqlx::{AssertSqlSafe, Row};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::written::{
    holder_read, holder_written, purpose_read, purpose_written, size_written, unreachable,
};

const HOLD: &str = "
    INSERT INTO asset_use_holders (purpose, holder_kind, holder, version, gone_at)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT (purpose, holder_kind, holder) DO UPDATE
    SET version = excluded.version, gone_at = excluded.gone_at
    WHERE asset_use_holders.version < excluded.version
    RETURNING version
";

const LET_GO: &str =
    "DELETE FROM asset_uses WHERE purpose = ?1 AND holder_kind = ?2 AND holder = ?3";

const USE: &str =
    "INSERT INTO asset_uses (purpose, holder_kind, holder, asset) VALUES (?1, ?2, ?3, ?4)";

const FORGET_OLD: &str = "
    DELETE FROM asset_use_holders WHERE (purpose, holder_kind, holder) IN (
        SELECT purpose, holder_kind, holder FROM asset_use_holders
        WHERE gone_at < ?1
        ORDER BY gone_at, purpose, holder_kind, holder
        LIMIT ?2
    )
";

/// What uses which asset, stored in Sqlite next to the assets.
/// `asset_use_holders` keeps each holder's version and tombstone, `asset_uses` the assets it uses.
#[derive(Debug, Default)]
pub struct SqliteAssetUses;

impl SqliteAssetUses {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AssetUses for SqliteAssetUses {
    async fn used_for(
        &self,
        transaction: &mut Transaction,
        purpose: Purpose,
        by: &Holder,
        assets: &BTreeSet<AssetId>,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let purpose = purpose_written(purpose);
        let (holder_kind, holder) = holder_written(by);
        let stored: Option<i64> = sqlx::query_scalar(HOLD)
            .bind(purpose)
            .bind(holder_kind)
            .bind(&holder)
            .bind(size_written(version.count())?)
            .bind(assets.is_empty().then(|| clock::text::written(at)))
            .fetch_optional(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        if stored.is_none() {
            return Ok(());
        }

        sqlx::query(LET_GO)
            .bind(purpose)
            .bind(holder_kind)
            .bind(&holder)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        for asset in assets {
            sqlx::query(USE)
                .bind(purpose)
                .bind(holder_kind)
                .bind(&holder)
                .bind(asset.to_string())
                .execute(connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?;
        }

        Ok(())
    }

    async fn uses_of(
        &self,
        transaction: &mut Transaction,
        assets: &[AssetId],
    ) -> Result<Vec<(AssetId, AssetUse)>, CatalogError> {
        if assets.is_empty() {
            return Ok(Vec::new());
        }
        let asked = format!(
            "SELECT asset, purpose, holder_kind, holder FROM asset_uses WHERE asset IN ({})",
            (1..=assets.len())
                .map(|nth| format!("?{nth}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut asking = sqlx::query(AssertSqlSafe(asked));
        for asset in assets {
            asking = asking.bind(asset.to_string());
        }
        let rows = asking
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        rows.iter()
            .map(|row| {
                let asset: String = row.try_get("asset").map_err(unreachable)?;
                let purpose: String = row.try_get("purpose").map_err(unreachable)?;
                let holder_kind: String = row.try_get("holder_kind").map_err(unreachable)?;
                let holder: String = row.try_get("holder").map_err(unreachable)?;
                Ok((
                    asset.parse().map_err(unreachable)?,
                    AssetUse {
                        purpose: purpose_read(&purpose)?,
                        by: holder_read(&holder_kind, &holder)?,
                    },
                ))
            })
            .collect()
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_OLD)
            .bind(clock::text::written(before))
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use sqlx::SqlitePool;
    use test_harness::SqliteFixture;
    use transacting::{SqliteTransactions, Transactions};

    use super::*;
    use crate::sqlite;
    use crate::uses_suite::Workbench;

    struct OnSqlite {
        fixture: SqliteFixture,
        store: SqliteAssetUses,
        transactions: SqliteTransactions,
    }

    #[async_trait]
    impl Workbench for OnSqlite {
        type Store = SqliteAssetUses;

        async fn setup() -> Self {
            let fixture = SqliteFixture::setup();
            let pool: SqlitePool = fixture.create_database("assets").await;
            sqlite::migrations()
                .run(&pool)
                .await
                .expect("the schema should lay down in an empty file");

            Self {
                fixture,
                store: SqliteAssetUses::new(),
                transactions: SqliteTransactions::new(pool),
            }
        }

        fn store(&self) -> &Self::Store {
            &self.store
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn cleanup(self) {
            self.fixture.cleanup().await;
        }
    }

    crate::uses_conformance_tests!(OnSqlite);
}
