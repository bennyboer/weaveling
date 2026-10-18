#![cfg(feature = "postgres")]

mod journeys;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use weaveling_service_api::{Storage, Unprepared};
use wiring::Databases;

use journeys::{Backend, DATABASES};

struct OnPostgres {
    fixture: PostgresFixture,
    schemas: SchemaEach,
}

#[derive(Clone)]
struct SchemaEach {
    pools: Arc<HashMap<&'static str, PgPool>>,
}

impl SchemaEach {
    fn of(&self, database: &str) -> &PgPool {
        self.pools
            .get(database)
            .unwrap_or_else(|| panic!("the fixture should have made a {database} schema"))
    }
}

#[async_trait]
impl Databases for SchemaEach {
    async fn ready(&self, feature: &str) -> Result<PgPool, Unprepared> {
        self.pools
            .get(feature)
            .cloned()
            .ok_or_else(|| Unprepared::Unreachable {
                database: feature.to_owned(),
                why: "the fixture made no schema for it".to_owned(),
            })
    }
}

#[async_trait]
impl Backend for OnPostgres {
    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let mut pools = HashMap::new();
        for database in DATABASES {
            pools.insert(database, fixture.create_schema(database).await);
        }

        Self {
            fixture,
            schemas: SchemaEach {
                pools: Arc::new(pools),
            },
        }
    }

    fn storage(&self) -> Storage {
        Storage::Postgres(Arc::new(self.schemas.clone()))
    }

    async fn read(&self, database: &str, query: &'static str) -> Vec<String> {
        sqlx::query_scalar(query)
            .fetch_all(self.schemas.of(database))
            .await
            .expect("reading should succeed")
    }

    async fn count(&self, database: &str, query: &'static str) -> i64 {
        sqlx::query_scalar(query)
            .fetch_one(self.schemas.of(database))
            .await
            .expect("counting should succeed")
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

journeys!(OnPostgres);
