use sqlx::PgPool;
use wiring::Unprepared;
use wiring::database::{connect, ensure};

const MESSAGING: &str = "messaging";

pub struct Databases {
    pub messaging: PgPool,
    pub projects: PgPool,
    pub ideas: PgPool,
    pub boards: PgPool,
    pub outline: PgPool,
    pub passages: PgPool,
    pub appearances: PgPool,
}

impl Databases {
    pub async fn ready(server: &str) -> Result<Self, Unprepared> {
        for feature in [
            projects_wiring::NAME,
            ideas_wiring::NAME,
            boards_wiring::NAME,
            outline_wiring::NAME,
            passages_wiring::NAME,
            appearances_wiring::NAME,
            MESSAGING,
        ] {
            ensure(server, feature).await?;
        }

        let databases = Self {
            messaging: connect(server, MESSAGING).await?,
            projects: connect(server, projects_wiring::NAME).await?,
            ideas: connect(server, ideas_wiring::NAME).await?,
            boards: connect(server, boards_wiring::NAME).await?,
            outline: connect(server, outline_wiring::NAME).await?,
            passages: connect(server, passages_wiring::NAME).await?,
            appearances: connect(server, appearances_wiring::NAME).await?,
        };
        databases.lay_out().await?;

        Ok(databases)
    }

    pub async fn lay_out(&self) -> Result<(), Unprepared> {
        projects_wiring::lay_out(&self.projects).await?;
        ideas_wiring::lay_out(&self.ideas).await?;
        boards_wiring::lay_out(&self.boards).await?;
        outline_wiring::lay_out(&self.outline).await?;
        passages_wiring::lay_out(&self.passages).await?;
        appearances_wiring::lay_out(&self.appearances).await?;
        wiring::database::lay_out(MESSAGING, &self.messaging, messaging::migrations()).await
    }
}
