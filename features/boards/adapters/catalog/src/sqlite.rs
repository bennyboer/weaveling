use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, IdeaLink, ProjectLink};
use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};

const REMEMBER: &str = "
    INSERT INTO board_summaries (board, project)
    VALUES (?1, ?2)
    ON CONFLICT (board) DO UPDATE SET project = excluded.project
";

const FORGET: &str = "DELETE FROM board_summaries WHERE board = ?1";

const IN_PROJECT: &str = "
    SELECT board, project
    FROM board_summaries
    WHERE project = ?1
    ORDER BY board
";

const LET_GO: &str = "DELETE FROM board_ideas WHERE board = ?1";

const HOLD: &str = "
    INSERT INTO board_ideas (board, idea)
    VALUES (?1, ?2)
    ON CONFLICT DO NOTHING
";

const HOLDING: &str = "SELECT board FROM board_ideas WHERE idea = ?1 ORDER BY board";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteBoardCatalog {
    pool: SqlitePool,
}

impl SqliteBoardCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl BoardCatalog for SqliteBoardCatalog {
    async fn remember(&self, summary: &BoardSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.project.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(&self, board: &BoardId) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(FORGET)
            .bind(board.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        sqlx::query(LET_GO)
            .bind(board.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<BoardSummary>, CatalogError> {
        let found = sqlx::query(IN_PROJECT)
            .bind(project.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let board: String = row.try_get("board").map_err(unreachable)?;
                let project: String = row.try_get("project").map_err(unreachable)?;

                Ok(BoardSummary {
                    id: board.parse().map_err(unreachable)?,
                    project: ProjectLink::from(project.as_str()),
                })
            })
            .collect()
    }

    async fn holds(&self, board: BoardId, ideas: &[IdeaLink]) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(LET_GO)
            .bind(board.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        for idea in ideas {
            sqlx::query(HOLD)
                .bind(board.to_string())
                .bind(idea.to_string())
                .execute(&mut *transaction)
                .await
                .map_err(unreachable)?;
        }

        transaction.commit().await.map_err(unreachable)
    }

    async fn boards_holding(&self, idea: &IdeaLink) -> Result<Vec<BoardId>, CatalogError> {
        let found = sqlx::query(HOLDING)
            .bind(idea.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let board: String = row.try_get("board").map_err(unreachable)?;

                board.parse().map_err(unreachable)
            })
            .collect()
    }
}
