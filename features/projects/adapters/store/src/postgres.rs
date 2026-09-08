use async_trait::async_trait;
use projects_core::{Project, ProjectId, ProjectName, ProjectStore, StoreError};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

const WRITE: &str = "
    INSERT INTO projects (project, name, created_at, updated_at)
    VALUES ($1, $2, $3, $4)
";

const READ: &str = "
    SELECT project, name, created_at, updated_at
    FROM projects
    WHERE project = $1
";

const READ_ALL: &str = "
    SELECT project, name, created_at, updated_at
    FROM projects
    ORDER BY project
";

const RENAME: &str = "UPDATE projects SET name = $2, updated_at = $3 WHERE project = $1";

const FORGET: &str = "DELETE FROM projects WHERE project = $1";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations")
}

pub struct PostgresProjectStore {
    pool: PgPool,
}

impl PostgresProjectStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: sqlx::Error) -> StoreError {
    StoreError::Backend(Box::new(failure))
}

fn is_taken(failure: &sqlx::Error) -> bool {
    matches!(failure, sqlx::Error::Database(reason) if reason.is_unique_violation())
}

fn stored(row: &PgRow) -> Result<Project, StoreError> {
    let id: String = row.try_get("project").map_err(unreachable)?;
    let name: String = row.try_get("name").map_err(unreachable)?;
    let created_at: OffsetDateTime = row.try_get("created_at").map_err(unreachable)?;
    let updated_at: OffsetDateTime = row.try_get("updated_at").map_err(unreachable)?;

    Ok(Project::from_parts(
        id.parse()
            .map_err(|reason| StoreError::Backend(Box::new(reason)))?,
        ProjectName::new(&name).map_err(|reason| StoreError::Backend(Box::new(reason)))?,
        created_at,
        updated_at,
    ))
}

#[async_trait]
impl ProjectStore for PostgresProjectStore {
    async fn create(&self, project: Project) -> Result<(), StoreError> {
        let id = project.id();

        sqlx::query(WRITE)
            .bind(id.to_string())
            .bind(project.name().as_str())
            .bind(project.created_at())
            .bind(project.updated_at())
            .execute(&self.pool)
            .await
            .map_err(|failure| match is_taken(&failure) {
                true => StoreError::Conflict(id),
                false => unreachable(failure),
            })?;

        Ok(())
    }

    async fn get(&self, id: ProjectId) -> Result<Project, StoreError> {
        let found = sqlx::query(READ)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        match found {
            Some(row) => stored(&row),
            None => Err(StoreError::NotFound(id)),
        }
    }

    async fn list(&self) -> Result<Vec<Project>, StoreError> {
        let found = sqlx::query(READ_ALL)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(stored).collect()
    }

    async fn update(&self, project: Project) -> Result<(), StoreError> {
        let id = project.id();
        let changed = sqlx::query(RENAME)
            .bind(id.to_string())
            .bind(project.name().as_str())
            .bind(project.updated_at())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match changed.rows_affected() {
            0 => Err(StoreError::NotFound(id)),
            _ => Ok(()),
        }
    }

    async fn delete(&self, id: ProjectId) -> Result<(), StoreError> {
        let gone = sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        match gone.rows_affected() {
            0 => Err(StoreError::NotFound(id)),
            _ => Ok(()),
        }
    }
}
