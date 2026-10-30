use async_trait::async_trait;
use clock::text;
use eventsourcing::{AccountId, Version};
use notifications_core::{CatalogError, NoticeCatalog, NoticeId, NoticeSummary};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::types::Json;
use sqlx::{Row, SqlitePool};

use crate::kept;
use crate::kept::KeptAbout;
use crate::kept::{unreachable, unreadable};

const REMEMBER: &str = "
    INSERT INTO notices (notice, version, account, project, about, noted_at, dismissed)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    ON CONFLICT (notice) DO UPDATE
    SET version   = excluded.version,
        account   = excluded.account,
        project   = excluded.project,
        about     = excluded.about,
        noted_at  = excluded.noted_at,
        dismissed = excluded.dismissed
    WHERE notices.version < excluded.version
";

const OPEN_FOR: &str = "
    SELECT notice, version, account, project, about, noted_at, dismissed
    FROM notices
    WHERE account = ?1 AND NOT dismissed
    ORDER BY noted_at DESC, notice DESC
";

const LEDGER: &str = "_sqlx_migrations_notices";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteNoticeCatalog {
    pool: SqlitePool,
}

impl SqliteNoticeCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn summary_of(row: &SqliteRow) -> Result<NoticeSummary, CatalogError> {
    let id: String = row.try_get("notice").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let noted_at: String = row.try_get("noted_at").map_err(unreachable)?;

    Ok(NoticeSummary {
        id: id
            .parse::<NoticeId>()
            .map_err(|_| unreadable(format!("{id:?} is no notice")))?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        account: AccountId::from(row.try_get::<String, _>("account").map_err(unreachable)?),
        about: kept::read(
            row.try_get("project").map_err(unreachable)?,
            row.try_get::<Json<KeptAbout>, _>("about")
                .map_err(unreachable)?
                .0,
        )?,
        noted_at: text::read(&noted_at)
            .ok_or_else(|| unreadable(format!("{noted_at:?} is no moment")))?,
        dismissed: row.try_get("dismissed").map_err(unreachable)?,
    })
}

#[async_trait]
impl NoticeCatalog for SqliteNoticeCatalog {
    async fn remember(&self, summary: &NoticeSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.account.to_string())
            .bind(kept::project_of(&summary.about))
            .bind(Json(kept::kept_as(&summary.about)))
            .bind(text::written(summary.noted_at))
            .bind(summary.dismissed)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn open_for(&self, account: &AccountId) -> Result<Vec<NoticeSummary>, CatalogError> {
        sqlx::query(OPEN_FOR)
            .bind(account.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(summary_of)
            .collect()
    }
}
