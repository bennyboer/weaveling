use std::sync::Arc;
use std::time::Duration;

use axum_test::{TestServer, TestWebSocket};
use live::Broadcaster;
use scenes_catalog::InMemorySceneCatalog;
use scenes_core::{ProjectLink, SceneCatalog, SceneId, SceneTitle};
use scenes_rest::TellingScenes;
use serde_json::{Value, json};
use time::OffsetDateTime;

struct ScenesFollowed {
    catalog: TellingScenes,
    socket: TestWebSocket,
    _server: TestServer,
}

async fn scenes_followed(project: &str) -> ScenesFollowed {
    let broadcaster = Broadcaster::new();
    let catalog = TellingScenes::around(Arc::new(InMemorySceneCatalog::new()), broadcaster.clone());
    let server = TestServer::builder()
        .http_transport()
        .build(scenes_rest::live_scenes(broadcaster));
    let socket = server
        .get_websocket(&format!("/scenes/live?project={project}"))
        .await
        .into_websocket()
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

async fn told(socket: &mut TestWebSocket) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
        .await
        .expect("a scene's summary is told while it is fresh");

    serde_json::from_str(&frame).expect("a frame is JSON")
}

async fn retitled(catalog: &TellingScenes, id: SceneId, title: &str, version: u64) {
    catalog
        .retitle(
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
    let mut followed = scenes_followed("project_1").await;
    let id = a_scene(1_000);

    followed
        .catalog
        .remember(id, &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");
    let made = told(&mut followed.socket).await;
    retitled(&followed.catalog, id, "The loom", 1).await;
    let named = told(&mut followed.socket).await;
    followed
        .catalog
        .note_opening(
            id,
            &ProjectLink::from("project_1"),
            "Once there was",
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("noting should succeed");
    let opened = told(&mut followed.socket).await;

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
    let mut followed = scenes_followed("project_1").await;
    let id = a_scene(1_000);
    retitled(&followed.catalog, id, "The loom, again", 2).await;
    told(&mut followed.socket).await;

    retitled(&followed.catalog, id, "The loom", 1).await;

    assert_eq!(
        told(&mut followed.socket).await["event"]["title"],
        "The loom, again"
    );
}

#[tokio::test]
async fn a_forgotten_scene_and_another_project_s_tell_nothing() {
    let mut followed = scenes_followed("project_1").await;
    let gone = a_scene(1_000);
    followed
        .catalog
        .forget(gone, OffsetDateTime::UNIX_EPOCH)
        .await
        .expect("forgetting should succeed");
    let theirs = a_scene(2_000);
    let mine = a_scene(3_000);

    retitled(&followed.catalog, gone, "Gone", 1).await;
    followed
        .catalog
        .remember(theirs, &ProjectLink::from("project_2"))
        .await
        .expect("remembering should succeed");
    retitled(&followed.catalog, mine, "Mine", 1).await;

    assert_eq!(
        told(&mut followed.socket).await["of"],
        mine.to_string(),
        "the first thing told is the followed project's scene that is still there"
    );
}

#[tokio::test]
async fn forgetting_old_tombstones_reaches_the_catalog() {
    let followed = scenes_followed("project_1").await;
    followed
        .catalog
        .forget(a_scene(1_000), OffsetDateTime::UNIX_EPOCH)
        .await
        .expect("forgetting should succeed");

    let forgotten = followed
        .catalog
        .forget_tombstones_laid_before(
            OffsetDateTime::UNIX_EPOCH + Duration::from_secs(1),
            u64::MAX,
        )
        .await
        .expect("forgetting tombstones should succeed");

    assert_eq!(forgotten, 1);
}
