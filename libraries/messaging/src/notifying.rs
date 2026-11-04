use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::postgres::{PgListener, PgPoolOptions};

use crate::listening::Notifications;

const CHANNEL: &str = "SELECT left($1 || current_schema(), 63)";

pub struct Listening(PgListener);

pub async fn listening_to(pool: &PgPool, prefix: &str) -> Result<Listening, sqlx::Error> {
    let channel: String = sqlx::query_scalar(CHANNEL)
        .bind(prefix)
        .fetch_one(pool)
        .await?;

    let own = PgPoolOptions::new()
        .max_connections(1)
        .max_lifetime(None)
        .idle_timeout(None)
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let mut listening = PgListener::connect_with(&own).await?;
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sqlx::postgres::PgPoolOptions;

    use super::*;

    #[tokio::test]
    async fn listening_leaves_the_pool_it_listens_beside_all_its_connections() {
        // given: a pool of one connection
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(1))
            .connect(&test_harness::database_url())
            .await
            .expect("postgres should be reachable");

        // when: something listens beside it
        let _listening = listening_to(&pool, "listening_beside_")
            .await
            .expect("listening should begin");

        // then: the pool still hands out its connection
        assert!(
            pool.acquire().await.is_ok(),
            "a listener holding a pooled connection for its whole life leaves the pool's work one connection short"
        );
    }
}
