use std::sync::Arc;

use axum_test::TestServer;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_contract::WordCountDTO;
use statistics_core::{ProjectLink, SceneLink, WordCountCatalog};
use time::OffsetDateTime;

#[tokio::test]
async fn a_project_nobody_counted_answers_with_nothing() {
    let server = TestServer::new(statistics_rest::router(Arc::new(
        InMemoryWordCountCatalog::new(),
    )));

    let response = server
        .get("/statistics/word-counts?project=project_1")
        .await;

    response.assert_status_ok();
    assert!(response.json::<Vec<WordCountDTO>>().is_empty());
}

#[tokio::test]
async fn a_project_answers_with_the_count_of_each_of_its_scenes() {
    let catalog = Arc::new(InMemoryWordCountCatalog::new());
    for (scene, project, words) in [
        ("scene_1", "project_1", 42),
        ("scene_2", "project_1", 7),
        ("scene_3", "project_2", 99),
    ] {
        catalog
            .keep(
                &SceneLink::from(scene),
                &ProjectLink::from(project),
                words,
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .expect("keeping should succeed");
    }
    let server = TestServer::new(statistics_rest::router(catalog));

    let response = server
        .get("/statistics/word-counts?project=project_1")
        .await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<Vec<WordCountDTO>>(),
        vec![
            WordCountDTO {
                scene: "scene_1".to_owned(),
                words: 42
            },
            WordCountDTO {
                scene: "scene_2".to_owned(),
                words: 7
            },
        ]
    );
}

#[tokio::test]
async fn asking_without_a_project_is_a_bad_request() {
    let server = TestServer::new(statistics_rest::router(Arc::new(
        InMemoryWordCountCatalog::new(),
    )));

    let response = server.get("/statistics/word-counts").expect_failure().await;

    response.assert_status_bad_request();
}
