use sqlx::PgPool;
use wiring::Unprepared;
use wiring::database::{connect, ensure};

pub struct Databases {
    pub projects: PgPool,
    pub pieces: PgPool,
    pub boards: PgPool,
    pub outline: PgPool,
    pub passages: PgPool,
}

impl Databases {
    pub async fn ready(server: &str) -> Result<Self, Unprepared> {
        for feature in [
            projects_wiring::NAME,
            pieces_wiring::NAME,
            boards_wiring::NAME,
            outline_wiring::NAME,
            passages_wiring::NAME,
        ] {
            ensure(server, feature).await?;
        }

        let databases = Self {
            projects: connect(server, projects_wiring::NAME).await?,
            pieces: connect(server, pieces_wiring::NAME).await?,
            boards: connect(server, boards_wiring::NAME).await?,
            outline: connect(server, outline_wiring::NAME).await?,
            passages: connect(server, passages_wiring::NAME).await?,
        };
        databases.lay_out().await?;

        Ok(databases)
    }

    pub async fn lay_out(&self) -> Result<(), Unprepared> {
        projects_wiring::lay_out(&self.projects).await?;
        pieces_wiring::lay_out(&self.pieces).await?;
        boards_wiring::lay_out(&self.boards).await?;
        outline_wiring::lay_out(&self.outline).await?;
        passages_wiring::lay_out(&self.passages).await
    }
}
