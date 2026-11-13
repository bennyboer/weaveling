use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use axum_test::TestServer;
use eventsourcing::Agent;
use live::Broadcaster;
use live::testing::Subscriber;
use serde_json::{Value, json};
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::{ProjectLink, SceneLink, WordCountCatalog};
use statistics_rest::TellingWordCounts;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction, Transactions};

struct CountsFollowed {
    catalog: TellingWordCounts,
    socket: Subscriber,
    _server: TestServer,
}

async fn counts_followed(project: &str) -> CountsFollowed {
    let broadcaster = Broadcaster::new();
    let catalog = TellingWordCounts::around(
        Arc::new(InMemoryWordCountCatalog::new()),
        broadcaster.clone(),
    );
    let server = TestServer::builder().http_transport().build(
        live::live_socket(live::Topics::of(vec![statistics_rest::topic(
            crate::allowing::everyone(
                Arc::new(InMemoryTransactions::new()),
                Arc::new(InMemoryWordCountCatalog::new()),
            ),
            broadcaster,
        )]))
        .layer(Extension(Agent::Local)),
    );
    let mut socket = Subscriber::on(&server).await;
    socket
        .subscribe("counts", "word-counts", &[("project", project)])
        .await;

    CountsFollowed {
        catalog,
        socket,
        _server: server,
    }
}

async fn counted_in(
    transaction: &mut Transaction,
    catalog: &TellingWordCounts,
    scene: &str,
    project: &str,
    words: u64,
    at: i64,
) {
    catalog
        .keep(
            transaction,
            &SceneLink::from(scene),
            &ProjectLink::from(project),
            words,
            OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(at),
        )
        .await
        .expect("keeping should succeed");
}

async fn counted(catalog: &TellingWordCounts, scene: &str, project: &str, words: u64, at: i64) {
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    counted_in(&mut transaction, catalog, scene, project, words, at).await;
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn forgotten(catalog: &TellingWordCounts, scene: &str) {
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    catalog
        .forget_scene(
            &mut transaction,
            &SceneLink::from(scene),
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("forgetting should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn told(socket: &mut Subscriber) -> Value {
    socket.frame().await
}

fn count_in(frame: &Value) -> (String, u64) {
    (
        frame["event"]["scene"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        frame["event"]["words"].as_u64().unwrap_or_default(),
    )
}

#[tokio::test]
async fn whoever_follows_a_project_s_counts_is_told_each_scene_s_as_it_is_kept() {
    // given: a project's counts followed
    let mut followed = counts_followed("project_1").await;

    // when: a count of one scene is kept and then of another
    counted(&followed.catalog, "scene_1", "project_1", 42, 1).await;
    let first = told(&mut followed.socket).await;
    counted(&followed.catalog, "scene_2", "project_1", 7, 2).await;
    let second = told(&mut followed.socket).await;

    // then: each is told as it was kept
    assert_eq!(count_in(&first), ("scene_1".to_owned(), 42));
    assert_eq!(count_in(&second), ("scene_2".to_owned(), 7));
    assert_eq!(second["of"], "scene_2");
    // and: the later telling is the newer
    assert!(
        second["version"].as_u64() > first["version"].as_u64(),
        "each telling is newer than the last"
    );
}

#[tokio::test]
async fn an_older_count_is_told_as_the_count_that_is_kept() {
    // given: a project's counts followed
    let mut followed = counts_followed("project_1").await;
    // and: a count of a scene told
    counted(&followed.catalog, "scene_1", "project_1", 42, 5).await;
    told(&mut followed.socket).await;

    // when: an older count of it is kept
    counted(&followed.catalog, "scene_1", "project_1", 3, 1).await;

    // then: the newer count is told
    assert_eq!(
        count_in(&told(&mut followed.socket).await),
        ("scene_1".to_owned(), 42),
        "what is told is what the catalog holds, not what came in"
    );
}

#[tokio::test]
async fn a_count_for_a_forgotten_scene_and_another_project_s_tell_nothing() {
    // given: a project's counts followed
    let mut followed = counts_followed("project_1").await;
    // and: a forgotten scene
    forgotten(&followed.catalog, "scene_gone").await;

    // when: the forgotten scene is counted
    counted(&followed.catalog, "scene_gone", "project_1", 5, 1).await;
    // and: a scene of another project
    counted(&followed.catalog, "scene_9", "project_2", 99, 2).await;
    // and: a scene of the followed project
    counted(&followed.catalog, "scene_1", "project_1", 42, 3).await;

    // then: only the followed project's scene is told
    assert_eq!(
        told(&mut followed.socket).await,
        json!({ "of": "scene_1", "version": 2, "event": { "scene": "scene_1", "words": 42 } }),
        "the first thing told is the followed project's scene that is still there"
    );
}

#[tokio::test]
async fn forgetting_old_tombstones_reaches_the_catalog() {
    // given: a forgotten scene in a telling catalog
    let followed = counts_followed("project_1").await;
    forgotten(&followed.catalog, "scene_gone").await;

    // when: old tombstones are forgotten through it
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    let forgotten = followed
        .catalog
        .forget_tombstones_laid_before(
            &mut transaction,
            OffsetDateTime::UNIX_EPOCH + Duration::from_secs(1),
            u64::MAX,
        )
        .await
        .expect("forgetting tombstones should succeed");

    // then: the scene's tombstone is forgotten
    assert_eq!(forgotten, 1);
}

#[tokio::test]
async fn a_count_kept_in_a_transaction_that_is_dropped_tells_nothing() {
    // given: a project's counts followed
    let mut followed = counts_followed("project_1").await;
    // and: a count kept in a transaction
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    counted_in(
        &mut transaction,
        &followed.catalog,
        "scene_1",
        "project_1",
        42,
        1,
    )
    .await;

    // when: the transaction is dropped
    drop(transaction);

    // then: nothing is told
    followed.socket.nothing().await;
}
