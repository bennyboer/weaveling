use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, IdeaLink, ProjectLink};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

const REMEMBER: &str = "
    INSERT INTO board_summaries (board, project)
    SELECT $1, $2
    WHERE NOT EXISTS (SELECT 1 FROM board_tombstones WHERE board = $1)
    ON CONFLICT (board) DO UPDATE SET project = EXCLUDED.project
";

const FORGET: &str = "DELETE FROM board_summaries WHERE board = $1";

const LAY_SUMMARY_TOMBSTONE: &str =
    "INSERT INTO board_tombstones (board, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET_OLD_SUMMARY_TOMBSTONES: &str = "
    DELETE FROM board_tombstones WHERE board IN (
        SELECT board FROM board_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, board
        LIMIT $2
    )
";

const HOLD_SUMMARY: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))";

const IN_PROJECT: &str = "
    SELECT board, project
    FROM board_summaries
    WHERE project = $1
    ORDER BY board
";

const LET_GO: &str = "DELETE FROM board_ideas WHERE board = $1";

const HOLD: &str = "
    INSERT INTO board_ideas (board, idea)
    SELECT $1, held
    FROM unnest($2::text[]) AS held
    ON CONFLICT DO NOTHING
";

const HOLDING: &str = "SELECT board FROM board_ideas WHERE idea = $1 ORDER BY board";

const LAY_TOMBSTONE: &str =
    "INSERT INTO idea_tombstones (idea, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET_OLD_TOMBSTONES: &str = "
    DELETE FROM idea_tombstones WHERE idea IN (
        SELECT idea FROM idea_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, idea
        LIMIT $2
    )
";

const HELD_THOUGH_TOMBSTONED: &str = "
    SELECT held.idea
    FROM board_ideas held
    JOIN idea_tombstones tombstone ON tombstone.idea = held.idea
    WHERE held.board = $1
    ORDER BY held.idea
";

const LAY_PROJECT_TOMBSTONE: &str =
    "INSERT INTO project_tombstones (project, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const HAS_PROJECT_TOMBSTONE: &str =
    "SELECT EXISTS (SELECT 1 FROM project_tombstones WHERE project = $1)";

const FORGET_OLD_PROJECT_TOMBSTONES: &str = "
    DELETE FROM project_tombstones WHERE project IN (
        SELECT project FROM project_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, project
        LIMIT $2
    )
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/postgres")
}

pub struct PostgresBoardCatalog {
    pool: PgPool,
}

impl PostgresBoardCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl BoardCatalog for PostgresBoardCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &BoardSummary,
    ) -> Result<(), CatalogError> {
        sqlx::query(HOLD_SUMMARY)
            .bind(summary.id.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.project.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        board: &BoardId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(HOLD_SUMMARY)
            .bind(board.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(LAY_SUMMARY_TOMBSTONE)
            .bind(board.to_string())
            .bind(at)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        sqlx::query(FORGET)
            .bind(board.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        sqlx::query(LET_GO)
            .bind(board.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
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

    async fn holds(
        &self,
        transaction: &mut Transaction,
        board: BoardId,
        ideas: &[IdeaLink],
    ) -> Result<(), CatalogError> {
        let held: Vec<String> = ideas.iter().map(ToString::to_string).collect();

        sqlx::query(LET_GO)
            .bind(board.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        sqlx::query(HOLD)
            .bind(board.to_string())
            .bind(&held)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
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

    async fn lay_tombstone(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_TOMBSTONE)
            .bind(idea.to_string())
            .bind(at)
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
        let pieces = sqlx::query(FORGET_OLD_TOMBSTONES)
            .bind(before)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected();
        let summaries = sqlx::query(FORGET_OLD_SUMMARY_TOMBSTONES)
            .bind(before)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected();

        let projects = sqlx::query(FORGET_OLD_PROJECT_TOMBSTONES)
            .bind(before)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected();

        Ok(pieces + summaries + projects)
    }

    async fn held_though_tombstoned(&self, board: BoardId) -> Result<Vec<IdeaLink>, CatalogError> {
        let found = sqlx::query(HELD_THOUGH_TOMBSTONED)
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

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_PROJECT_TOMBSTONE)
            .bind(project.to_string())
            .bind(at)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        sqlx::query_scalar(HAS_PROJECT_TOMBSTONE)
            .bind(project.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }
}
