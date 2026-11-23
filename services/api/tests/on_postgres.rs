#![cfg(feature = "postgres")]

mod journeys;
mod mail_drop;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use weaveling_service_api::{Storage, Unprepared};
use wiring::Databases;

use journeys::{Backend, DATABASES, Journey, id_in, until};

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

#[tokio::test(flavor = "multi_thread")]
async fn a_server_keeps_an_address_signed_up_in_the_accounts_database() {
    // given: a server with accounts
    let journey = Journey::<OnPostgres>::with_accounts().await;

    // when: an address signs up
    journey
        .server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
        .await
        .assert_status(StatusCode::ACCEPTED);

    // then: the accounts database keeps it and the one event
    assert_eq!(
        journey
            .backend
            .read("accounts", "SELECT email FROM account_email_addresses")
            .await,
        vec!["ada@example.com".to_owned()]
    );
    assert_eq!(
        journey
            .backend
            .count("accounts", "SELECT count(*) FROM events")
            .await,
        1
    );
    journey.end().await;
}

// TODO: among the journeys of every backend once the feedback is kept on SQLite too (5d3)
#[tokio::test(flavor = "multi_thread")]
async fn a_draft_in_a_reading_copy_is_kept_in_the_feedback_database() {
    // given: a reading copy of a project on a relayed server
    let journey = Journey::<OnPostgres>::relayed().await;
    let project = journey.project("Kept feedback").await;
    let copy = id_in(
        &journey
            .posted(
                "/api/readings/copies",
                json!({
                    "project": project,
                    "name": "For Lena",
                    "shared": { "kind": "book" },
                    "open_until": null,
                    "deleted_on": null,
                }),
                StatusCode::CREATED,
            )
            .await,
        "id",
    );

    // when: a draft is begun in it, once the feedback has heard of the copy
    let draft = json!({
        "about": { "kind": "reading_copy", "reading_copy": copy, "place": { "kind": "whole" } },
        "comment": [{ "kind": "paragraph", "runs": [{ "kind": "text", "text": "Lovely." }] }],
    });
    assert!(
        until(|| async {
            journey
                .server
                .post("/api/feedback")
                .json(&draft)
                .await
                .status_code()
                == StatusCode::CREATED
        })
        .await,
        "the feedback should hear of the copy, and of the project it was made of, and take the draft"
    );

    // then: the feedback database keeps the piece and its event
    assert_eq!(
        journey
            .backend
            .count("feedback", "SELECT count(*) FROM feedback")
            .await,
        1
    );
    assert_eq!(
        journey
            .backend
            .count("feedback", "SELECT count(*) FROM events")
            .await,
        1
    );
    // and: it is listed as the draft it is
    assert_eq!(
        journey
            .read(&format!(
                "/api/feedback/drafts?subjects=reading_copy:{copy}"
            ))
            .await[0]["comment"][0]["runs"][0]["text"],
        "Lovely."
    );
    journey.end().await;
}
