use assets_core::{AssetCatalog, AssetId, AssetSummary, CatalogError, ProjectLink, Usage};
use async_trait::async_trait;
use eventsourcing::AccountId;
use sqlx::Row;
use sqlx::migrate::Migrator;
use transacting::Transaction;
use transacting::postgres::{Connection, connection};

use crate::written::{
    ACCOUNT, INSTANCE, PROJECT, bounds_of, format_written, owner_written, size_written,
    unreachable, usage_of,
};

const REMEMBER: &str = "
    INSERT INTO asset_summaries
        (id, owner_kind, owner, uploader, format, width, height, original_size, total_size, uploaded_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
    ON CONFLICT (id) DO UPDATE SET
        owner_kind = excluded.owner_kind,
        owner = excluded.owner,
        uploader = excluded.uploader,
        format = excluded.format,
        width = excluded.width,
        height = excluded.height,
        original_size = excluded.original_size,
        total_size = excluded.total_size,
        uploaded_at = excluded.uploaded_at
";

const FORGET: &str = "DELETE FROM asset_summaries WHERE id = $1";

const HELD: &str =
    "SELECT uploader, owner_kind, owner, total_size FROM asset_summaries WHERE id = $1";

const ADD: &str = "
    INSERT INTO asset_usage (bound, id, total) VALUES ($1, $2, $3)
    ON CONFLICT (bound, id) DO UPDATE SET total = asset_usage.total + excluded.total
";

const TAKE_OFF: &str = "UPDATE asset_usage SET total = total - $3 WHERE bound = $1 AND id = $2";

const USAGE: &str = "
    SELECT bound, total FROM asset_usage
    WHERE (bound = $1 AND id = '')
       OR (bound = $2 AND id = $3)
       OR (bound = $4 AND id = $5)
";

const LEDGER: &str = "_sqlx_migrations_asset_summaries";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

/// The assets on PostgreSQL, with a running total for the installation, each account and each project, so what an upload is weighed against is read by its keys and never summed over every asset.
#[derive(Debug, Default)]
pub struct PostgresAssetCatalog;

impl PostgresAssetCatalog {
    pub fn new() -> Self {
        Self
    }
}

async fn counted(
    connection: &mut Connection,
    query: &'static str,
    uploader: Option<&str>,
    owner_kind: &str,
    owner: &str,
    total_size: i64,
) -> Result<(), CatalogError> {
    for (bound, id) in bounds_of(uploader, owner_kind, owner) {
        sqlx::query(query)
            .bind(bound)
            .bind(id)
            .bind(total_size)
            .execute(&mut *connection)
            .await
            .map_err(unreachable)?;
    }

    Ok(())
}

/// Takes what an asset counts off every total it counts in, where it is held.
async fn taken_off(connection: &mut Connection, id: &AssetId) -> Result<(), CatalogError> {
    let held = sqlx::query(HELD)
        .bind(id.to_string())
        .fetch_optional(&mut *connection)
        .await
        .map_err(unreachable)?;
    let Some(held) = held else {
        return Ok(());
    };
    let uploader: Option<String> = held.try_get("uploader").map_err(unreachable)?;
    let owner_kind: String = held.try_get("owner_kind").map_err(unreachable)?;
    let owner: String = held.try_get("owner").map_err(unreachable)?;
    let total_size: i64 = held.try_get("total_size").map_err(unreachable)?;

    counted(
        connection,
        TAKE_OFF,
        uploader.as_deref(),
        &owner_kind,
        &owner,
        total_size,
    )
    .await
}

#[async_trait]
impl AssetCatalog for PostgresAssetCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AssetSummary,
    ) -> Result<(), CatalogError> {
        let connection = connection(transaction).map_err(unreachable)?;
        taken_off(connection, &summary.id).await?;
        let (owner_kind, owner) = owner_written(&summary.owner);
        let uploader = summary.uploader.as_ref().map(ToString::to_string);
        let total_size = size_written(summary.total_size)?;
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(owner_kind)
            .bind(&owner)
            .bind(&uploader)
            .bind(format_written(summary.format))
            .bind(i64::from(summary.width))
            .bind(i64::from(summary.height))
            .bind(size_written(summary.original_size)?)
            .bind(total_size)
            .bind(summary.uploaded_at)
            .execute(&mut *connection)
            .await
            .map_err(unreachable)?;

        counted(
            connection,
            ADD,
            uploader.as_deref(),
            owner_kind,
            &owner,
            total_size,
        )
        .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: &AssetId,
    ) -> Result<(), CatalogError> {
        let connection = connection(transaction).map_err(unreachable)?;
        taken_off(connection, id).await?;
        sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&mut *connection)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn usage_within(
        &self,
        transaction: &mut Transaction,
        uploader: Option<&AccountId>,
        project: Option<&ProjectLink>,
    ) -> Result<Usage, CatalogError> {
        let rows = sqlx::query(USAGE)
            .bind(INSTANCE)
            .bind(ACCOUNT)
            .bind(uploader.map(ToString::to_string))
            .bind(PROJECT)
            .bind(project.map(ToString::to_string))
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        usage_of(
            rows.iter()
                .map(|row| Ok((row.try_get("bound")?, row.try_get("total")?)))
                .collect::<Result<Vec<_>, sqlx::Error>>()
                .map_err(unreachable)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ledger_is_the_catalogs_own() {
        // expect: the ledger is neither the default one nor the contents'
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(migrations().table_name, Migrator::DEFAULT.table_name);
        assert_ne!(
            migrations().table_name,
            "_sqlx_migrations_asset_contents",
            "the contents lay their table down in the same schema"
        );
    }
}
