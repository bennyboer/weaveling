#![cfg(feature = "postgres")]

use sqlx::{PgPool, Row};
use weaveling_scratch::Scratch;

async fn tables_in(pool: &PgPool, schema: &str) -> Vec<String> {
    sqlx::query("select tablename::text from pg_tables where schemaname = $1 order by tablename")
        .bind(schema)
        .fetch_all(pool)
        .await
        .expect("reading the catalog should succeed")
        .into_iter()
        .map(|row| row.get::<String, _>(0))
        .collect()
}

async fn rows_of_events(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from events")
        .fetch_one(pool)
        .await
        .expect("counting should succeed")
}

async fn an_event(pool: &PgPool, aggregate: &str) {
    sqlx::query(
        "insert into events (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot)
         values ($1, 'SAMPLE', 1, 'CREATED', '{}'::jsonb, 0, 'anonymous', now(), false)",
    )
    .bind(aggregate)
    .execute(pool)
    .await
    .expect("writing an event should succeed");
}

#[tokio::test]
async fn a_scratch_schema_arrives_already_migrated() {
    let scratch = Scratch::fresh().await;

    let found = tables_in(scratch.pool(), scratch.schema()).await;

    assert!(
        found.contains(&"events".to_owned()) && found.contains(&"outbox".to_owned()),
        "the migrations should have run inside the scratch schema, found {found:?}"
    );
    scratch.discard().await;
}

#[tokio::test]
async fn the_migrations_land_in_the_scratch_schema_and_not_in_public() {
    let scratch = Scratch::fresh().await;

    let elsewhere = tables_in(scratch.pool(), "public").await;

    assert!(
        !elsewhere.contains(&"events".to_owned()),
        "a test that writes into public would be visible to every other test"
    );
    scratch.discard().await;
}

#[tokio::test]
async fn two_scratches_cannot_see_each_others_writing() {
    let mine = Scratch::fresh().await;
    let yours = Scratch::fresh().await;

    an_event(mine.pool(), "sample_1").await;

    assert_eq!(rows_of_events(mine.pool()).await, 1);
    assert_eq!(
        rows_of_events(yours.pool()).await,
        0,
        "isolation is what lets the suites run in parallel"
    );

    mine.discard().await;
    yours.discard().await;
}

#[tokio::test]
async fn a_discarded_scratch_leaves_nothing_behind() {
    let scratch = Scratch::fresh().await;
    let schema = scratch.schema().to_owned();
    let looking = Scratch::fresh().await;

    scratch.discard().await;

    assert!(
        tables_in(looking.pool(), &schema).await.is_empty(),
        "a discarded schema should be gone, not merely emptied"
    );
    looking.discard().await;
}
