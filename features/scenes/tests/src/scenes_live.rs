use std::sync::Arc;
use std::time::Duration;

use authorizing::AllowingEverything;
use axum::Extension;
use axum_test::TestServer;
use belonging::InMemoryBelongings;
use clock::FixedClock;
use eventsourcing::Agent;
use live::Broadcaster;
use live::testing::Subscriber;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{ProjectLink, SceneCatalog, SceneId, SceneTitle, ScenesCore};
use scenes_rest::TellingScenes;
use scenes_store::InMemorySceneStore;
use serde_json::{Value, json};
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transactions};

use crate::committed::Committed;

struct ScenesFollowed {
    catalog: TellingScenes,
    socket: Subscriber,
    _server: TestServer,
}

async fn scenes_followed(project: &str) -> ScenesFollowed {
    let broadcaster = Broadcaster::new();
    let catalog = TellingScenes::around(Arc::new(InMemorySceneCatalog::new()), broadcaster.clone());
    let scenes = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    let server = TestServer::builder().http_transport().build(
        live::live_socket(live::Topics::of(vec![scenes_rest::scenes_topic(
            scenes,
            broadcaster,
        )]))
        .layer(Extension(Agent::Local)),
    );
    let mut socket = Subscriber::on(&server).await;
    socket
        .subscribe("scenes", "scenes", &[("project", project)])
        .await;

    ScenesFollowed {
        catalog,
        socket,
        _server: server,
    }
}

fn a_scene(made_at: i64) -> SceneId {
    SceneId::generate(OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(made_at))
}

async fn told(socket: &mut Subscriber) -> Value {
    socket.frame().await
}

async fn retitled(catalog: &TellingScenes, id: SceneId, title: &str, version: u64) {
    catalog
        .retitle_committed(
            id,
            &ProjectLink::from("project_1"),
            &SceneTitle::new(title).expect("a plain title"),
            version,
        )
        .await
        .expect("retitling should succeed");
}

#[tokio::test]
async fn whoever_follows_a_project_s_scenes_is_told_each_scene_as_it_is_kept() {
    // given: a project's scenes followed
    let mut followed = scenes_followed("project_1").await;
    let id = a_scene(1_000);

    // when: a scene of it is kept
    followed
        .catalog
        .remember_committed(id, &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");
    let made = told(&mut followed.socket).await;
    // and: retitled
    retitled(&followed.catalog, id, "The loom", 1).await;
    let named = told(&mut followed.socket).await;
    // and: given an opening
    followed
        .catalog
        .note_opening_committed(
            id,
            &ProjectLink::from("project_1"),
            "Once there was",
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("noting should succeed");
    let opened = told(&mut followed.socket).await;

    // then: the follower is told the scene each time, as it then stands
    assert_eq!(made["of"], id.to_string());
    assert_eq!(
        named["event"],
        json!({ "id": id.to_string(), "project": "project_1", "title": "The loom", "opening": "" })
    );
    assert_eq!(opened["event"]["opening"], "Once there was");
    assert_eq!(opened["event"]["title"], "The loom");
    assert!(opened["version"].as_u64() > named["version"].as_u64());
}

#[tokio::test]
async fn a_late_retitle_is_told_as_the_title_that_is_kept() {
    // given: a followed scene retitled
    let mut followed = scenes_followed("project_1").await;
    let id = a_scene(1_000);
    retitled(&followed.catalog, id, "The loom, again", 2).await;
    told(&mut followed.socket).await;

    // when: an older retitle arrives late
    retitled(&followed.catalog, id, "The loom", 1).await;

    // then: the follower is told the newer title
    assert_eq!(
        told(&mut followed.socket).await["event"]["title"],
        "The loom, again"
    );
}

#[tokio::test]
async fn a_forgotten_scene_and_another_project_s_tell_nothing() {
    // given: a project's scenes followed
    let mut followed = scenes_followed("project_1").await;
    // and: a forgotten scene of it
    let gone = a_scene(1_000);
    followed
        .catalog
        .forget_committed(gone, OffsetDateTime::UNIX_EPOCH)
        .await
        .expect("forgetting should succeed");
    let theirs = a_scene(2_000);
    let mine = a_scene(3_000);

    // when: the forgotten scene is retitled
    retitled(&followed.catalog, gone, "Gone", 1).await;
    // and: another project's scene is kept
    followed
        .catalog
        .remember_committed(theirs, &ProjectLink::from("project_2"))
        .await
        .expect("remembering should succeed");
    // and: a scene of the followed project is retitled
    retitled(&followed.catalog, mine, "Mine", 1).await;

    // then: only the followed project's scene is told
    assert_eq!(
        told(&mut followed.socket).await["of"],
        mine.to_string(),
        "the first thing told is the followed project's scene that is still there"
    );
}

#[tokio::test]
async fn forgetting_old_tombstones_reaches_the_catalog() {
    // given: a scene forgotten, leaving a tombstone
    let followed = scenes_followed("project_1").await;
    followed
        .catalog
        .forget_committed(a_scene(1_000), OffsetDateTime::UNIX_EPOCH)
        .await
        .expect("forgetting should succeed");

    // when: tombstones laid before now are forgotten through the telling catalog
    let forgotten = followed
        .catalog
        .forget_tombstones_laid_before_committed(
            OffsetDateTime::UNIX_EPOCH + Duration::from_secs(1),
            u64::MAX,
        )
        .await
        .expect("forgetting tombstones should succeed");

    // then: the tombstone is forgotten
    assert_eq!(forgotten, 1);
}

#[tokio::test]
async fn a_scene_catalogued_in_a_transaction_that_is_dropped_tells_nobody() {
    // given: a project's scenes followed
    let mut followed = scenes_followed("project_1").await;
    // and: a scene of it catalogued in an open transaction
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    followed
        .catalog
        .remember(
            &mut transaction,
            a_scene(1_000),
            &ProjectLink::from("project_1"),
        )
        .await
        .expect("remembering should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: nobody is told
    assert!(followed.socket.silent().await);
}
