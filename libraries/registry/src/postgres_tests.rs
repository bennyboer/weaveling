use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::Registry;
use crate::postgres::{PostgresRegistry, migrations};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresRegistry,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresRegistry;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("registry").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresRegistry::new(pool.clone()),
            pool,
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnPostgres);

#[tokio::test]
async fn a_race_for_one_key_leaves_exactly_one_winner() {
    let bench = OnPostgres::setup().await;
    let registry = std::sync::Arc::new(PostgresRegistry::new(bench.pool.clone()));

    let racing: Vec<_> = (1..=8)
        .map(|nth| {
            let registry = registry.clone();

            tokio::spawn(async move {
                registry
                    .claim("board", "project_1", &format!("board_{nth}"))
                    .await
                    .expect("claiming should succeed")
            })
        })
        .collect();

    let mut held = Vec::new();
    for attempt in racing {
        held.push(attempt.await.expect("the claim should not panic"));
    }

    let first = held.first().expect("eight claims were made").clone();
    assert!(
        held.iter().all(|held| held == &first),
        "eight concurrent opens must all be told the same board, got {held:?}"
    );

    bench.cleanup().await;
}
