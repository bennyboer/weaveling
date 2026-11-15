use std::collections::BTreeSet;

use assets_core::{AssetId, AssetUse, AssetUses, CatalogError, Holder, Purpose};
use async_trait::async_trait;
use eventsourcing::Version;
use sqlx::Row;
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

use crate::written::{
    holder_read, holder_written, purpose_read, purpose_written, size_written, unreachable,
};

const HOLD: &str = "
    INSERT INTO asset_use_holders (purpose, holder_kind, holder, version, gone_at)
    VALUES ($1, $2, $3, $4, $5)
    ON CONFLICT (purpose, holder_kind, holder) DO UPDATE
    SET version = excluded.version, gone_at = excluded.gone_at
    WHERE asset_use_holders.version < excluded.version
    RETURNING version
";

const LET_GO: &str =
    "DELETE FROM asset_uses WHERE purpose = $1 AND holder_kind = $2 AND holder = $3";

const USE: &str = "
    INSERT INTO asset_uses (purpose, holder_kind, holder, asset)
    SELECT $1, $2, $3, UNNEST($4::TEXT[])
";

const USES_OF: &str =
    "SELECT asset, purpose, holder_kind, holder FROM asset_uses WHERE asset = ANY($1)";

const FORGET_OLD: &str = "
    DELETE FROM asset_use_holders WHERE (purpose, holder_kind, holder) IN (
        SELECT purpose, holder_kind, holder FROM asset_use_holders
        WHERE gone_at < $1
        ORDER BY gone_at, purpose, holder_kind, holder
        LIMIT $2
    )
";

/// What uses which asset, stored in Postgres next to the assets.
/// `asset_use_holders` keeps each holder's version and tombstone, `asset_uses` the assets it uses.
#[derive(Debug, Default)]
pub struct PostgresAssetUses;

impl PostgresAssetUses {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AssetUses for PostgresAssetUses {
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
            .bind(assets.is_empty().then_some(at))
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
        sqlx::query(USE)
            .bind(purpose)
            .bind(holder_kind)
            .bind(&holder)
            .bind(assets.iter().map(ToString::to_string).collect::<Vec<_>>())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn uses_of(
        &self,
        transaction: &mut Transaction,
        assets: &[AssetId],
    ) -> Result<Vec<(AssetId, AssetUse)>, CatalogError> {
        let rows = sqlx::query(USES_OF)
            .bind(assets.iter().map(ToString::to_string).collect::<Vec<_>>())
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
            .bind(before)
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
    use sqlx::PgPool;
    use test_harness::PostgresFixture;
    use transacting::{PostgresTransactions, Transactions};

    use super::*;
    use crate::postgres;
    use crate::uses_suite::Workbench;

    struct OnPostgres {
        fixture: PostgresFixture,
        store: PostgresAssetUses,
        transactions: PostgresTransactions,
    }

    #[async_trait]
    impl Workbench for OnPostgres {
        type Store = PostgresAssetUses;

        async fn setup() -> Self {
            let fixture = PostgresFixture::setup().await;
            let pool: PgPool = fixture.create_schema("assets").await;
            postgres::migrations()
                .run(&pool)
                .await
                .expect("the schema should lay down in an empty namespace");

            Self {
                fixture,
                store: PostgresAssetUses::new(),
                transactions: PostgresTransactions::new(pool),
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

    crate::uses_conformance_tests!(OnPostgres);
}
