use async_trait::async_trait;
use eventsourcing::Version;
use pieces_core::{
    CatalogError, PassageLink, PieceCatalog, PieceId, PieceSummary, PieceTitle, ProjectLink,
};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

const REMEMBER: &str = "
    INSERT INTO piece_summaries (piece, version, project, title, passage)
    VALUES ($1, $2, $3, $4, $5)
    ON CONFLICT (piece) DO UPDATE
    SET version = EXCLUDED.version,
        project = EXCLUDED.project,
        title   = EXCLUDED.title,
        passage = EXCLUDED.passage
";

const FORGET: &str = "DELETE FROM piece_summaries WHERE piece = $1";

const IN_PROJECT: &str = "
    SELECT piece, version, project, title, passage
    FROM piece_summaries
    WHERE project = $1
    ORDER BY piece DESC
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations")
}

pub struct PostgresPieceCatalog {
    pool: PgPool,
}

impl PostgresPieceCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn summarised(row: &PgRow) -> Result<PieceSummary, CatalogError> {
    let piece: String = row.try_get("piece").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let project: String = row.try_get("project").map_err(unreachable)?;
    let title: String = row.try_get("title").map_err(unreachable)?;
    let passage: Option<String> = row.try_get("passage").map_err(unreachable)?;

    Ok(PieceSummary {
        id: piece.parse().map_err(unreachable)?,
        version: Version::of(version as u64),
        project: ProjectLink::from(project.as_str()),
        title: PieceTitle::new(&title).map_err(unreachable)?,
        passage: passage.map(|link| PassageLink::from(link.as_str())),
    })
}

#[async_trait]
impl PieceCatalog for PostgresPieceCatalog {
    async fn remember(&self, summary: &PieceSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.version.count() as i64)
            .bind(summary.project.to_string())
            .bind(summary.title.as_str())
            .bind(summary.passage.as_ref().map(ToString::to_string))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(&self, id: &PieceId) -> Result<(), CatalogError> {
        sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<PieceSummary>, CatalogError> {
        let found = sqlx::query(IN_PROJECT)
            .bind(project.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(summarised).collect()
    }
}
