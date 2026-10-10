use messaging::{Message, RoutingKey};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use time::{Duration, OffsetDateTime};

use crate::outbox::Origin;
use crate::sqlite::enqueue;

async fn ready(fixture: &SqliteFixture) -> SqlitePool {
    let pool = fixture.create_database("outbox").await;
    crate::sqlite::migrations()
        .run(&pool)
        .await
        .expect("the outbox's schema should lay down in an empty file");

    pool
}

fn a_link_message() -> Message {
    Message::opening(
        RoutingKey::parse("scene.idea.linked").expect("a declared routing key is fine"),
        serde_json::json!({ "scene": "scene_1", "idea": "idea_1" }),
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
    )
}

fn from_a_scene() -> Origin<'static> {
    Origin {
        aggregate: "scene_1",
        kind: "scene",
        version: 0,
    }
}

async fn waiting(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT routing_key FROM outbox WHERE published_at IS NULL ORDER BY entry")
        .fetch_all(pool)
        .await
        .expect("reading the outbox should succeed")
}

#[tokio::test]
async fn a_message_enqueued_beside_any_write_waits_once_that_write_commits() {
    let fixture = SqliteFixture::setup();
    let pool = ready(&fixture).await;
    let message = a_link_message();
    let mut transaction = pool.begin().await.expect("a transaction opens");

    enqueue(&mut transaction, from_a_scene(), &message)
        .await
        .expect("enqueueing should succeed");
    transaction.commit().await.expect("the transaction commits");

    let (id, payload): (String, String) =
        sqlx::query_as("SELECT message_id, payload FROM outbox WHERE published_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("the message should be waiting");
    assert_eq!(
        id,
        message.id.as_uuid().hyphenated().to_string(),
        "a retried message must keep the id it was written with"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&payload).expect("the payload is json"),
        message.payload
    );

    fixture.cleanup().await;
}

#[tokio::test]
async fn a_message_enqueued_beside_a_write_that_rolls_back_goes_with_it() {
    let fixture = SqliteFixture::setup();
    let pool = ready(&fixture).await;
    let mut transaction = pool.begin().await.expect("a transaction opens");

    enqueue(&mut transaction, from_a_scene(), &a_link_message())
        .await
        .expect("enqueueing should succeed");
    transaction
        .rollback()
        .await
        .expect("the transaction rolls back");

    assert!(
        waiting(&pool).await.is_empty(),
        "a change that never happened is never announced"
    );

    fixture.cleanup().await;
}

#[tokio::test]
async fn the_outbox_keeps_its_own_ledger_beside_whoever_owns_the_file() {
    let fixture = SqliteFixture::setup();
    let pool = ready(&fixture).await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(found, vec!["_sqlx_migrations_outbox", "outbox"]);

    fixture.cleanup().await;
}

#[tokio::test]
async fn the_schema_carries_the_indexes_the_queries_rely_on() {
    let fixture = SqliteFixture::setup();
    let pool = ready(&fixture).await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(found, vec!["outbox_published", "outbox_waiting"]);

    fixture.cleanup().await;
}
