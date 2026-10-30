use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use notifications_core::{
    CatalogError, NoticeCatalog, NoticeId, NoticeSummary, Person, ProjectLink, ProjectName,
};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::types::Json;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::kept;
use crate::kept::KeptAbout;
use crate::kept::{unreachable, unreadable};

const REMEMBER: &str = "
    INSERT INTO notices (notice, version, account, project, about, noted_at, dismissed)
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    ON CONFLICT (notice) DO UPDATE
    SET version   = EXCLUDED.version,
        account   = EXCLUDED.account,
        project   = EXCLUDED.project,
        about     = EXCLUDED.about,
        noted_at  = EXCLUDED.noted_at,
        dismissed = EXCLUDED.dismissed
    WHERE notices.version < EXCLUDED.version
";

const OPEN_FOR: &str = "
    SELECT notice, version, account, project, about, noted_at, dismissed
    FROM notices
    WHERE account = $1 AND NOT dismissed
      AND (project IS NULL OR project NOT IN (SELECT project FROM project_tombstones))
    ORDER BY noted_at DESC, notice DESC
";

const REMEMBER_PERSON: &str = "
    INSERT INTO people (account, name)
    VALUES ($1, $2)
    ON CONFLICT (account) DO UPDATE
    SET name = EXCLUDED.name
";

const PEOPLE: &str = "
    SELECT account, name
    FROM people
    WHERE account = ANY($1)
    ORDER BY account
";

const REMEMBER_PROJECT_NAME: &str = "
    INSERT INTO project_names (project, version, name)
    SELECT $1, $2, $3
    WHERE NOT EXISTS (SELECT 1 FROM project_tombstones WHERE project = $1)
    ON CONFLICT (project) DO UPDATE
    SET version = EXCLUDED.version,
        name    = EXCLUDED.name
    WHERE project_names.version < EXCLUDED.version
";

const PROJECT_NAMES: &str = "
    SELECT project, version, name
    FROM project_names
    WHERE project = ANY($1)
    ORDER BY project
";

const LAY_PROJECT_TOMBSTONE: &str =
    "INSERT INTO project_tombstones (project, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET_PROJECT_NAME: &str = "DELETE FROM project_names WHERE project = $1";

const HAS_PROJECT_TOMBSTONE: &str =
    "SELECT EXISTS (SELECT 1 FROM project_tombstones WHERE project = $1)";

const LEDGER: &str = "_sqlx_migrations_notices";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresNoticeCatalog {
    pool: PgPool,
}

impl PostgresNoticeCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn summary_of(row: &PgRow) -> Result<NoticeSummary, CatalogError> {
    let id: String = row.try_get("notice").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;

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
        noted_at: row
            .try_get::<OffsetDateTime, _>("noted_at")
            .map_err(unreachable)?,
        dismissed: row.try_get("dismissed").map_err(unreachable)?,
    })
}

#[async_trait]
impl NoticeCatalog for PostgresNoticeCatalog {
    async fn remember(&self, summary: &NoticeSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.account.to_string())
            .bind(kept::project_of(&summary.about))
            .bind(Json(kept::kept_as(&summary.about)))
            .bind(summary.noted_at)
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
            .bind(accounts.iter().map(ToString::to_string).collect::<Vec<_>>())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row: &PgRow| {
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
            .bind(projects.iter().map(ToString::to_string).collect::<Vec<_>>())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row: &PgRow| {
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
            .bind(at)
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
}
