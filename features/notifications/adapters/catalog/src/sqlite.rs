use async_trait::async_trait;
use clock::text;
use eventsourcing::{AccountId, Version};
use notifications_core::{
    CatalogError, NoticeCatalog, NoticeId, NoticeSummary, Person, ProjectLink, ProjectName,
};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::types::Json;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

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
      AND (project IS NULL OR project NOT IN (SELECT project FROM project_tombstones))
    ORDER BY noted_at DESC, notice DESC
";

const REMEMBER_PERSON: &str = "
    INSERT INTO people (account, name)
    VALUES (?1, ?2)
    ON CONFLICT (account) DO UPDATE
    SET name = excluded.name
";

const PEOPLE: &str = "
    SELECT account, name
    FROM people
    WHERE account IN (SELECT value FROM json_each(?1))
    ORDER BY account
";

const REMEMBER_PROJECT_NAME: &str = "
    INSERT INTO project_names (project, version, name)
    SELECT ?1, ?2, ?3
    WHERE NOT EXISTS (SELECT 1 FROM project_tombstones WHERE project = ?1)
    ON CONFLICT (project) DO UPDATE
    SET version = excluded.version,
        name    = excluded.name
    WHERE project_names.version < excluded.version
";

const PROJECT_NAMES: &str = "
    SELECT project, version, name
    FROM project_names
    WHERE project IN (SELECT value FROM json_each(?1))
    ORDER BY project
";

const LAY_PROJECT_TOMBSTONE: &str =
    "INSERT INTO project_tombstones (project, laid_at) VALUES (?1, ?2) ON CONFLICT DO NOTHING";

const FORGET_PROJECT_NAME: &str = "DELETE FROM project_names WHERE project = ?1";

const HAS_PROJECT_TOMBSTONE: &str =
    "SELECT EXISTS (SELECT 1 FROM project_tombstones WHERE project = ?1)";

const ACCOUNTS_NOTICED_ABOUT: &str = "
    SELECT DISTINCT account
    FROM notices
    WHERE project = ?1 AND NOT dismissed
    ORDER BY account
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

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_PERSON)
            .bind(person.account.to_string())
            .bind(&person.name)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        sqlx::query(PEOPLE)
            .bind(Json(
                accounts.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row: &SqliteRow| {
                Ok(Person {
                    account: AccountId::from(
                        row.try_get::<String, _>("account").map_err(unreachable)?,
                    ),
                    name: row.try_get("name").map_err(unreachable)?,
                })
            })
            .collect()
    }

    async fn remember_project_name(&self, named: &ProjectName) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_PROJECT_NAME)
            .bind(named.project.to_string())
            .bind(i64::try_from(named.version.count()).map_err(unreachable)?)
            .bind(&named.name)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError> {
        sqlx::query(PROJECT_NAMES)
            .bind(Json(
                projects.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row: &SqliteRow| {
                let version: i64 = row.try_get("version").map_err(unreachable)?;

                Ok(ProjectName {
                    project: ProjectLink::from(
                        row.try_get::<String, _>("project")
                            .map_err(unreachable)?
                            .as_str(),
                    ),
                    version: Version::of(u64::try_from(version).map_err(unreachable)?),
                    name: row.try_get("name").map_err(unreachable)?,
                })
            })
            .collect()
    }

    async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;
        sqlx::query(LAY_PROJECT_TOMBSTONE)
            .bind(project.to_string())
            .bind(text::written(at))
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET_PROJECT_NAME)
            .bind(project.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        sqlx::query_scalar(HAS_PROJECT_TOMBSTONE)
            .bind(project.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }

    async fn accounts_noticed_about(
        &self,
        project: &ProjectLink,
    ) -> Result<Vec<AccountId>, CatalogError> {
        Ok(sqlx::query_scalar::<_, String>(ACCOUNTS_NOTICED_ABOUT)
            .bind(project.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .into_iter()
            .map(AccountId::from)
            .collect())
    }
}
