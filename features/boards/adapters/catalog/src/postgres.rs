use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, PieceLink, ProjectLink};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};

const REMEMBER: &str = "
    INSERT INTO board_summaries (board, project)
    VALUES ($1, $2)
    ON CONFLICT (board) DO UPDATE SET project = EXCLUDED.project
";

const IN_PROJECT: &str = "
    SELECT board, project
    FROM board_summaries
    WHERE project = $1
    ORDER BY board
";

const LET_GO: &str = "DELETE FROM board_pieces WHERE board = $1";

const HOLD: &str = "
    INSERT INTO board_pieces (board, piece)
    SELECT $1, held
    FROM unnest($2::text[]) AS held
    ON CONFLICT DO NOTHING
";

const HOLDING: &str = "SELECT board FROM board_pieces WHERE piece = $1 ORDER BY board";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations")
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
    async fn remember(&self, summary: &BoardSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.project.to_string())
            .execute(&self.pool)
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

    async fn holds(&self, board: BoardId, pieces: &[PieceLink]) -> Result<(), CatalogError> {
        let held: Vec<String> = pieces.iter().map(ToString::to_string).collect();
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(LET_GO)
            .bind(board.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        sqlx::query(HOLD)
            .bind(board.to_string())
            .bind(&held)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn boards_holding(&self, piece: &PieceLink) -> Result<Vec<BoardId>, CatalogError> {
        let found = sqlx::query(HOLDING)
            .bind(piece.to_string())
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
