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

const TOLD: &str = "
    INSERT INTO asset_uses (purpose, holder_kind, holder, version, asset, gone_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
    ON CONFLICT (purpose, holder_kind, holder) DO UPDATE
    SET version = excluded.version, asset = excluded.asset, gone_at = excluded.gone_at
    WHERE asset_uses.version < excluded.version
";

const FORGET_OLD: &str = "
    DELETE FROM asset_uses WHERE (purpose, holder_kind, holder) IN (
        SELECT purpose, holder_kind, holder FROM asset_uses
        WHERE gone_at < ?1
        ORDER BY gone_at, purpose, holder_kind, holder
        LIMIT ?2
    )
";

/// What uses which asset, stored in Sqlite next to the assets.
#[derive(Debug, Default)]
pub struct SqliteAssetUses;

impl SqliteAssetUses {
    pub fn new() -> Self {
        Self
    }
}

async fn told(
    transaction: &mut Transaction,
    purpose: Purpose,
    by: &Holder,
    version: Version,
    asset: Option<&AssetId>,
    gone_at: Option<OffsetDateTime>,
) -> Result<(), CatalogError> {
    let (holder_kind, holder) = holder_written(by);
    sqlx::query(TOLD)
        .bind(purpose_written(purpose))
        .bind(holder_kind)
        .bind(holder)
        .bind(size_written(version.count())?)
        .bind(asset.map(ToString::to_string))
        .bind(gone_at.map(clock::text::written))
        .execute(connection(transaction).map_err(unreachable)?)
        .await
        .map_err(unreachable)?;

    Ok(())
}

#[async_trait]
impl AssetUses for SqliteAssetUses {
    async fn use_as(
        &self,
        transaction: &mut Transaction,
        asset: &AssetId,
        purpose: Purpose,
        by: &Holder,
        version: Version,
    ) -> Result<(), CatalogError> {
        told(transaction, purpose, by, version, Some(asset), None).await
    }

    async fn stop_using(
        &self,
        transaction: &mut Transaction,
        purpose: Purpose,
        by: &Holder,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        told(transaction, purpose, by, version, None, Some(at)).await
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
