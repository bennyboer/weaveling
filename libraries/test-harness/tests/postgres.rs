#![cfg(feature = "postgres")]

use sqlx::{PgPool, Row};
use weaveling_test_harness::PostgresFixture;

async fn a_table_of_notes(pool: &PgPool) {
    sqlx::query("CREATE TABLE notes (what TEXT)")
        .execute(pool)
        .await
        .expect("creating a table should succeed");
}

async fn note(pool: &PgPool, what: &str) {
    sqlx::query("INSERT INTO notes (what) VALUES ($1)")
        .bind(what)
        .execute(pool)
        .await
        .expect("writing a note should succeed");
}

async fn notes(pool: &PgPool) -> Vec<String> {
    sqlx::query("SELECT what FROM notes ORDER BY what")
        .fetch_all(pool)
        .await
        .expect("reading notes should succeed")
        .into_iter()
        .map(|row| row.get::<String, _>(0))
        .collect()
}

async fn tables_in(pool: &PgPool, schema: &str) -> Vec<String> {
    sqlx::query("SELECT tablename::text FROM pg_tables WHERE schemaname = $1")
        .bind(schema)
        .fetch_all(pool)
        .await
        .expect("reading the catalog should succeed")
        .into_iter()
        .map(|row| row.get::<String, _>(0))
        .collect()
}

#[tokio::test]
async fn a_feature_is_handed_an_empty_schema_of_its_own() {
    // given: a fixture
    let fixture = PostgresFixture::setup().await;

    // when: a feature asks for a schema
    let pool = fixture.create_schema("outline").await;

    // then: it is empty
    assert!(
        tables_in(&pool, &fixture.schema_of("outline"))
            .await
            .is_empty(),
        "a fixture schema arrives empty — what goes in it is the caller's own schema"
    );
    fixture.cleanup().await;
}

#[tokio::test]
async fn two_features_of_one_fixture_cannot_reach_each_other() {
    // given: two features' schemas of one fixture
    let fixture = PostgresFixture::setup().await;
    let outline = fixture.create_schema("outline").await;
    let ideas = fixture.create_schema("ideas").await;
    // and: a table of the same name in each
    a_table_of_notes(&outline).await;
    a_table_of_notes(&ideas).await;

    // when: one writes a note
    note(&outline, "in the book").await;

    // then: it reads its note
    assert_eq!(notes(&outline).await, vec!["in the book".to_owned()]);
    // and: the other reads none
    assert!(
        notes(&ideas).await.is_empty(),
        "one feature's rows are not another's, which is the whole point of the split"
    );

    fixture.cleanup().await;
}

#[tokio::test]
async fn nothing_written_in_one_fixture_is_visible_in_another() {
    // given: one feature's schema in each of two fixtures
    let (mine, yours) = PostgresFixture::two().await;
    let ours = mine.create_schema("outline").await;
    let theirs = yours.create_schema("outline").await;
    // and: a table of the same name in each
    a_table_of_notes(&ours).await;
    a_table_of_notes(&theirs).await;

    // when: one writes a note
    note(&ours, "mine alone").await;

    // then: it reads its note
    assert_eq!(notes(&ours).await.len(), 1);
    // and: the other reads none
    assert!(
        notes(&theirs).await.is_empty(),
        "isolation is what lets the suites run in parallel"
    );

    mine.cleanup().await;
    yours.cleanup().await;
}

#[tokio::test]
async fn a_cleaned_up_fixture_takes_every_schema_it_made_with_it() {
    // given: a fixture with two schemas, one holding a table
    let (fixture, looking) = PostgresFixture::two().await;
    let outline = fixture.create_schema("outline").await;
    let _ideas = fixture.create_schema("ideas").await;
    let named = [fixture.schema_of("outline"), fixture.schema_of("ideas")];
    a_table_of_notes(&outline).await;
    // and: another fixture looking on
    let elsewhere = looking.create_schema("watching").await;

    // when: the first is cleaned up
    fixture.cleanup().await;

    // then: none of its schemas is left
    for schema in named {
        assert!(
            tables_in(&elsewhere, &schema).await.is_empty(),
            "{schema} should be gone, not merely emptied"
        );
    }

    looking.cleanup().await;
}

#[tokio::test]
async fn a_schema_keeps_nothing_in_public() {
    // given: a feature's schema
    let fixture = PostgresFixture::setup().await;
    let pool = fixture.create_schema("outline").await;

    // when: a table is made in it
    a_table_of_notes(&pool).await;

    // then: public does not hold it
    assert!(
        !tables_in(&pool, "public")
            .await
            .contains(&"notes".to_owned()),
        "a test that writes into public would be visible to every other test"
    );
    fixture.cleanup().await;
}
