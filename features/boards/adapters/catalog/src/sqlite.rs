use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, IdeaLink, ProjectLink};
use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

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

const GONE: &str = "INSERT INTO gone_ideas (idea, gone_at) VALUES (?1, ?2) ON CONFLICT DO NOTHING";

const FORGET_LONG_GONE: &str = "
    DELETE FROM gone_ideas WHERE idea IN (
        SELECT idea FROM gone_ideas
        WHERE gone_at < ?1
        ORDER BY gone_at, idea
        LIMIT ?2
    )
";

const GONE_BUT_HELD: &str = "
    SELECT held.idea
    FROM board_ideas held
    JOIN gone_ideas gone ON gone.idea = held.idea
    WHERE held.board = ?1
    ORDER BY held.idea
";

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

    async fn mark_gone(&self, idea: &IdeaLink, at: OffsetDateTime) -> Result<(), CatalogError> {
        sqlx::query(GONE)
            .bind(idea.to_string())
            .bind(clock::text::written(at))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_LONG_GONE)
            .bind(clock::text::written(before))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn gone_but_held(&self, board: BoardId) -> Result<Vec<IdeaLink>, CatalogError> {
        let found = sqlx::query(GONE_BUT_HELD)
            .bind(board.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let idea: String = row.try_get("idea").map_err(unreachable)?;

                Ok(IdeaLink::from(idea.as_str()))
            })
            .collect()
    }
}
