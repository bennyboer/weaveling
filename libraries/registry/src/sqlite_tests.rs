use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::Registry;
use crate::sqlite::{SqliteRegistry, migrations};
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteRegistry,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteRegistry;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("registry").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteRegistry::new(pool.clone()),
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

crate::conformance_tests!(OnSqlite);

#[tokio::test]
async fn a_race_for_one_key_leaves_exactly_one_winner() {
    let bench = OnSqlite::setup().await;
    let registry = std::sync::Arc::new(SqliteRegistry::new(bench.pool.clone()));

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
