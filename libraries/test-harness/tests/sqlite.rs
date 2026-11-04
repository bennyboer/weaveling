use sqlx::SqlitePool;
use weaveling_test_harness::SqliteFixture;

async fn a_table_of_notes(pool: &SqlitePool) {
    sqlx::query("CREATE TABLE notes (note TEXT NOT NULL)")
        .execute(pool)
        .await
        .expect("should create the table");
}

async fn tables_named(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE name = ?")
        .bind(table)
        .fetch_one(pool)
        .await
        .expect("should count")
}

#[tokio::test]
async fn a_feature_is_handed_a_file_of_its_own() {
    // given: a fixture
    let fixture = SqliteFixture::setup();

    // when: a feature asks for a database
    let _ideas = fixture.create_database("ideas").await;

    // then: it is a file of its own
    assert!(fixture.directory().join("ideas.sqlite").is_file());
    fixture.cleanup().await;
}

#[tokio::test]
async fn two_features_of_one_fixture_cannot_reach_each_other() {
    // given: two features' databases of one fixture
    let fixture = SqliteFixture::setup();
    let ideas = fixture.create_database("ideas").await;
    let boards = fixture.create_database("boards").await;

    // when: one makes a table
    a_table_of_notes(&ideas).await;

    // then: the other has no such table
    assert_eq!(
        tables_named(&boards, "notes").await,
        0,
        "one feature's tables are not another's, which is the whole point of the split"
    );
    fixture.cleanup().await;
}

#[tokio::test]
async fn two_fixtures_made_together_never_share_a_directory() {
    // given: two fixtures made together
    let mine = SqliteFixture::setup();
    let yours = SqliteFixture::setup();

    // then: their directories differ
    assert_ne!(mine.directory(), yours.directory());
    mine.cleanup().await;
    yours.cleanup().await;
}

#[tokio::test]
async fn a_cleaned_up_fixture_takes_its_files_with_it() {
    // given: a fixture with a database holding a table
    let fixture = SqliteFixture::setup();
    let ideas = fixture.create_database("ideas").await;
    a_table_of_notes(&ideas).await;
    let directory = fixture.directory().to_path_buf();

    // when: it is cleaned up
    fixture.cleanup().await;

    // then: its directory is gone
    assert!(
        !directory.exists(),
        "an open pool would keep the file locked on Windows, so cleanup must close them first"
    );
}

#[tokio::test]
async fn a_fixture_database_is_opened_the_way_local_mode_opens_one() {
    // given: a fixture database
    let fixture = SqliteFixture::setup();
    let pool = fixture.create_database("ideas").await;

    // when: its settings are asked for
    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("should ask");
    let keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .expect("should ask");

    // then: they are local mode's
    assert_eq!(
        (journal.as_str(), keys),
        ("wal", 1),
        "a suite that passes against different settings than wiring::sqlite proves nothing about local mode"
    );
    fixture.cleanup().await;
}
