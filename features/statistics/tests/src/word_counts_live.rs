use std::sync::Arc;
use std::time::Duration;

use axum_test::{TestServer, TestWebSocket};
use live::Broadcaster;
use serde_json::{Value, json};
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::{ProjectLink, SceneLink, WordCountCatalog};
use statistics_rest::TellingWordCounts;
use time::OffsetDateTime;

struct CountsFollowed {
    catalog: TellingWordCounts,
    socket: TestWebSocket,
    _server: TestServer,
}

async fn counts_followed(project: &str) -> CountsFollowed {
    let broadcaster = Broadcaster::new();
    let catalog = TellingWordCounts::around(
        Arc::new(InMemoryWordCountCatalog::new()),
        broadcaster.clone(),
    );
    let server = TestServer::builder()
        .http_transport()
        .build(statistics_rest::live(broadcaster));
    let socket = server
        .get_websocket(&format!("/statistics/word-counts/live?project={project}"))
        .await
        .into_websocket()
        .await;

    CountsFollowed {
        catalog,
        socket,
        _server: server,
    }
}

async fn counted(catalog: &TellingWordCounts, scene: &str, project: &str, words: u64, at: i64) {
    catalog
        .keep(
            &SceneLink::from(scene),
            &ProjectLink::from(project),
            words,
            OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(at),
        )
        .await
        .expect("keeping should succeed");
}

async fn told(socket: &mut TestWebSocket) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
        .await
        .expect("the counts are told while they are fresh");

    serde_json::from_str(&frame).expect("a frame is JSON")
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
    let mut followed = counts_followed("project_1").await;

    counted(&followed.catalog, "scene_1", "project_1", 42, 1).await;
    let first = told(&mut followed.socket).await;
    counted(&followed.catalog, "scene_2", "project_1", 7, 2).await;
    let second = told(&mut followed.socket).await;

    assert_eq!(count_in(&first), ("scene_1".to_owned(), 42));
    assert_eq!(count_in(&second), ("scene_2".to_owned(), 7));
    assert_eq!(second["of"], "scene_2");
    assert!(
        second["version"].as_u64() > first["version"].as_u64(),
        "each telling is newer than the last"
    );
}

#[tokio::test]
async fn an_older_count_is_told_as_the_count_that_is_kept() {
    let mut followed = counts_followed("project_1").await;
    counted(&followed.catalog, "scene_1", "project_1", 42, 5).await;
    told(&mut followed.socket).await;

    counted(&followed.catalog, "scene_1", "project_1", 3, 1).await;

    assert_eq!(
        count_in(&told(&mut followed.socket).await),
        ("scene_1".to_owned(), 42),
        "what is told is what the catalog holds, not what came in"
    );
}

#[tokio::test]
async fn a_count_for_a_forgotten_scene_and_another_project_s_tell_nothing() {
    let mut followed = counts_followed("project_1").await;
    followed
        .catalog
        .forget_scene(&SceneLink::from("scene_gone"))
        .await
        .expect("forgetting should succeed");

    counted(&followed.catalog, "scene_gone", "project_1", 5, 1).await;
    counted(&followed.catalog, "scene_9", "project_2", 99, 2).await;
    counted(&followed.catalog, "scene_1", "project_1", 42, 3).await;

    assert_eq!(
        told(&mut followed.socket).await,
        json!({ "of": "scene_1", "version": 2, "event": { "scene": "scene_1", "words": 42 } }),
        "the first thing told is the followed project's scene that is still there"
    );
}
