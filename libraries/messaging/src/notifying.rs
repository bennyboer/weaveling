use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::postgres::PgListener;

use crate::listening::Notifications;

const CHANNEL: &str = "SELECT left($1 || current_schema(), 63)";

pub struct Listening(PgListener);

pub async fn listening_to(pool: &PgPool, prefix: &str) -> Result<Listening, sqlx::Error> {
    let channel: String = sqlx::query_scalar(CHANNEL)
        .bind(prefix)
        .fetch_one(pool)
        .await?;

    let mut listening = PgListener::connect_with(pool).await?;
    listening.listen(&channel).await?;

    Ok(Listening(listening))
}

#[async_trait]
impl Notifications for Listening {
    async fn wait(&mut self) {
        if let Err(why) = self.0.recv().await {
            tracing::warn!(error = %why, "a notification channel dropped, so polling carries on");
        }
    }
}
