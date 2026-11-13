use std::collections::HashSet;

use assets_core::{
    AssetCatalog, AssetId, AssetSummary, CatalogError, Held, Holders, Owner, Person, ProjectLink,
    Sorted, Upload, UploadMark, UploadsSorted, Usage,
};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use sqlx::AssertSqlSafe;
use sqlx::migrate::Migrator;
use sqlx::{Row, SqliteConnection};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::written::{
    ACCOUNT, INSTANCE, PROJECT, bounds_of, format_read, format_written, holdings_asked, owner_read,
    owner_written, pattern_of, person_searched, project_searched, size_written, unreachable,
    uploads_asked, usage_of,
};

const REMEMBER: &str = "
    INSERT INTO asset_summaries
        (id, owner_kind, owner, uploader, format, width, height, original_size, total_size, uploaded_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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

const FORGET: &str = "DELETE FROM asset_summaries WHERE id = ?1";

const OWNED: &str = "
    SELECT id FROM asset_summaries
    WHERE owner_kind = ?1 AND owner = ?2
    ORDER BY id
    LIMIT ?3
";

const HELD: &str =
    "SELECT uploader, owner_kind, owner, total_size FROM asset_summaries WHERE id = ?1";

const ADD: &str = "
    INSERT INTO asset_usage (bound, id, total) VALUES (?1, ?2, ?3)
    ON CONFLICT (bound, id) DO UPDATE SET total = asset_usage.total + excluded.total
";

const TAKE_OFF: &str = "UPDATE asset_usage SET total = total - ?3 WHERE bound = ?1 AND id = ?2";

const USAGE: &str = "
    SELECT bound, total FROM asset_usage
    WHERE (bound = ?1 AND id = '')
       OR (bound = ?2 AND id = ?3)
       OR (bound = ?4 AND id = ?5)
";

const PERSON: &str = "
    INSERT INTO asset_people (account, version, name, email, searched, gone_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
    ON CONFLICT (account) DO UPDATE
    SET version = excluded.version, name = excluded.name, email = excluded.email,
        searched = excluded.searched, gone_at = excluded.gone_at
    WHERE asset_people.version < excluded.version
";

const PROJECT_NAME: &str = "
    INSERT INTO asset_project_names (project, version, name, searched, gone_at)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT (project) DO UPDATE
    SET version = excluded.version, name = excluded.name,
        searched = excluded.searched, gone_at = excluded.gone_at
    WHERE asset_project_names.version < excluded.version
";

const FORGET_OLD_PEOPLE: &str = "
    DELETE FROM asset_people WHERE account IN (
        SELECT account FROM asset_people
        WHERE gone_at < ?1
        ORDER BY gone_at, account
        LIMIT ?2
    )
";

const FORGET_OLD_PROJECT_NAMES: &str = "
    DELETE FROM asset_project_names WHERE project IN (
        SELECT project FROM asset_project_names
        WHERE gone_at < ?1
        ORDER BY gone_at, project
        LIMIT ?2
    )
";

fn placeholder(nth: usize) -> String {
    format!("?{nth}")
}

const LEDGER: &str = "_sqlx_migrations_asset_summaries";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

/// The assets, stored in SQLite.
/// It keeps a running total for the installation, for each account and for each project.
/// So the usage an upload is checked against is read by key, and never summed over all assets.
#[derive(Debug, Default)]
pub struct SqliteAssetCatalog;

impl SqliteAssetCatalog {
    pub fn new() -> Self {
        Self
    }
}

async fn counted(
    connection: &mut SqliteConnection,
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

/// Subtracts an asset's size from every total it counts in.
/// Does nothing if the asset is not stored.
async fn taken_off(connection: &mut SqliteConnection, id: &AssetId) -> Result<(), CatalogError> {
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
impl AssetCatalog for SqliteAssetCatalog {
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
            .bind(clock::text::written(summary.uploaded_at))
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

    async fn held_by(
        &self,
        transaction: &mut Transaction,
        holders: Holders,
        sorted: Sorted,
        after: Option<&Held>,
        matching: Option<&str>,
        at_most: u64,
    ) -> Result<Vec<Held>, CatalogError> {
        let bound = match holders {
            Holders::Accounts => ACCOUNT,
            Holders::Projects => PROJECT,
        };
        let asked = holdings_asked(placeholder, holders, sorted, after, matching.is_some());
        let mut asking = sqlx::query(AssertSqlSafe(asked))
            .bind(bound)
            .bind(i64::try_from(at_most).map_err(unreachable)?);
        if let Some(last) = after {
            if sorted == Sorted::MostUsed {
                asking = asking.bind(size_written(last.total)?);
            }
            asking = asking.bind(last.id.clone());
        }
        if let Some(term) = matching {
            asking = asking.bind(pattern_of(term));
        }
        let rows = asking
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        rows.iter()
            .map(|row| {
                let total: i64 = row.try_get("total").map_err(unreachable)?;
                Ok(Held {
                    id: row.try_get("id").map_err(unreachable)?,
                    total: u64::try_from(total).map_err(unreachable)?,
                    name: row.try_get("name").map_err(unreachable)?,
                    email: row.try_get("email").map_err(unreachable)?,
                })
            })
            .collect()
    }

    async fn held_of(
        &self,
        transaction: &mut Transaction,
        ids: &[AssetId],
    ) -> Result<HashSet<AssetId>, CatalogError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        let asked = format!(
            "SELECT id FROM asset_summaries WHERE id IN ({})",
            (1..=ids.len())
                .map(placeholder)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut asking = sqlx::query(AssertSqlSafe(asked));
        for id in ids {
            asking = asking.bind(id.to_string());
        }
        let rows = asking
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        rows.iter()
            .map(|row| {
                let id: String = row.try_get("id").map_err(unreachable)?;
                id.parse().map_err(unreachable)
            })
            .collect()
    }

    async fn owned_by(
        &self,
        transaction: &mut Transaction,
        owner: &Owner,
        at_most: u64,
    ) -> Result<Vec<AssetId>, CatalogError> {
        let (owner_kind, owner) = owner_written(owner);
        let rows = sqlx::query(OWNED)
            .bind(owner_kind)
            .bind(owner)
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        rows.iter()
            .map(|row| {
                let id: String = row.try_get("id").map_err(unreachable)?;
                id.parse().map_err(unreachable)
            })
            .collect()
    }

    async fn uploaded_by(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        sorted: UploadsSorted,
        after: Option<&UploadMark>,
        at_most: u64,
    ) -> Result<Vec<Upload>, CatalogError> {
        let asked = uploads_asked(placeholder, sorted, after.is_some());
        let mut asking = sqlx::query(AssertSqlSafe(asked))
            .bind(account.to_string())
            .bind(i64::try_from(at_most).map_err(unreachable)?);
        if let Some(last) = after {
            asking = match sorted {
                UploadsSorted::Largest => asking.bind(size_written(last.total_size)?),
                UploadsSorted::Newest => asking.bind(clock::text::written(last.uploaded_at)),
            };
            asking = asking.bind(last.id.to_string());
        }
        let rows = asking
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        rows.iter()
            .map(|row| {
                let id: String = row.try_get("id").map_err(unreachable)?;
                let owner_kind: String = row.try_get("owner_kind").map_err(unreachable)?;
                let owner: String = row.try_get("owner").map_err(unreachable)?;
                let format: String = row.try_get("format").map_err(unreachable)?;
                let width: i64 = row.try_get("width").map_err(unreachable)?;
                let height: i64 = row.try_get("height").map_err(unreachable)?;
                let total_size: i64 = row.try_get("total_size").map_err(unreachable)?;
                let uploaded_at: String = row.try_get("uploaded_at").map_err(unreachable)?;
                Ok(Upload {
                    id: id.parse().map_err(unreachable)?,
                    owner: owner_read(&owner_kind, &owner)?,
                    project_name: row.try_get("project_name").map_err(unreachable)?,
                    format: format_read(&format)?,
                    width: u32::try_from(width).map_err(unreachable)?,
                    height: u32::try_from(height).map_err(unreachable)?,
                    total_size: u64::try_from(total_size).map_err(unreachable)?,
                    uploaded_at: clock::text::read(&uploaded_at).ok_or_else(|| {
                        CatalogError::Backend(format!("no moment is {uploaded_at}").into())
                    })?,
                })
            })
            .collect()
    }

    async fn people(
        &self,
        transaction: &mut Transaction,
        accounts: &[AccountId],
    ) -> Result<Vec<Person>, CatalogError> {
        if accounts.is_empty() {
            return Ok(Vec::new());
        }
        let asked = format!(
            "SELECT account, name, email FROM asset_people WHERE name IS NOT NULL AND account IN ({})",
            (1..=accounts.len())
                .map(placeholder)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut asking = sqlx::query(AssertSqlSafe(asked));
        for account in accounts {
            asking = asking.bind(account.to_string());
        }
        let rows = asking
            .fetch_all(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        rows.iter()
            .map(|row| {
                let account: String = row.try_get("account").map_err(unreachable)?;
                Ok(Person {
                    account: AccountId::from(account),
                    name: row.try_get("name").map_err(unreachable)?,
                    email: row.try_get("email").map_err(unreachable)?,
                })
            })
            .collect()
    }

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
        version: Version,
    ) -> Result<(), CatalogError> {
        sqlx::query(PERSON)
            .bind(person.account.to_string())
            .bind(size_written(version.count())?)
            .bind(&person.name)
            .bind(&person.email)
            .bind(person_searched(&person.name, &person.email))
            .bind(None::<OffsetDateTime>)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_person(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(PERSON)
            .bind(account.to_string())
            .bind(size_written(version.count())?)
            .bind(None::<String>)
            .bind(None::<String>)
            .bind(None::<String>)
            .bind(Some(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        name: &str,
    ) -> Result<(), CatalogError> {
        sqlx::query(PROJECT_NAME)
            .bind(project.to_string())
            .bind(size_written(version.count())?)
            .bind(name)
            .bind(project_searched(name))
            .bind(None::<OffsetDateTime>)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(PROJECT_NAME)
            .bind(project.to_string())
            .bind(size_written(version.count())?)
            .bind(None::<String>)
            .bind(None::<String>)
            .bind(Some(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let at_most = i64::try_from(at_most).map_err(unreachable)?;
        let mut forgotten = 0;
        for forgetting in [FORGET_OLD_PEOPLE, FORGET_OLD_PROJECT_NAMES] {
            forgotten += sqlx::query(forgetting)
                .bind(before)
                .bind(at_most)
                .execute(connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?
                .rows_affected();
        }

        Ok(forgotten)
    }
}
