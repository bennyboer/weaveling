use std::sync::Arc;

use axum::Extension;
use axum_test::TestServer;
use eventsourcing::Agent;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_contract::WordCountDTO;
use statistics_core::{ProjectLink, SceneLink, WordCountCatalog};
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transactions};

#[tokio::test]
async fn a_project_nobody_counted_answers_with_nothing() {
    // given: nothing counted
    let server = TestServer::new(
        statistics_rest::router(crate::allowing::everyone(
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryWordCountCatalog::new()),
        ))
        .layer(Extension(Agent::Local)),
    );

    // when: a project's counts are asked for
    let response = server
        .get("/statistics/word-counts?project=project_1")
        .await;

    // then: the answer is empty
    response.assert_status_ok();
    assert!(response.json::<Vec<WordCountDTO>>().is_empty());
}

#[tokio::test]
async fn a_project_answers_with_the_count_of_each_of_its_scenes() {
    // given: counts of scenes in two projects
    let catalog = Arc::new(InMemoryWordCountCatalog::new());
    for (scene, project, words) in [
        ("scene_1", "project_1", 42),
        ("scene_2", "project_1", 7),
        ("scene_3", "project_2", 99),
    ] {
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .expect("beginning should succeed");
        catalog
            .keep(
                &mut transaction,
                &SceneLink::from(scene),
                &ProjectLink::from(project),
                words,
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .expect("keeping should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");
    }
    let server = TestServer::new(
        statistics_rest::router(crate::allowing::everyone(
            Arc::new(InMemoryTransactions::new()),
            catalog,
        ))
        .layer(Extension(Agent::Local)),
    );

    // when: one project's counts are asked for
    let response = server
        .get("/statistics/word-counts?project=project_1")
        .await;

    // then: the answer is the count of each of its scenes
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
    // given: the statistics
    let server = TestServer::new(
        statistics_rest::router(crate::allowing::everyone(
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryWordCountCatalog::new()),
        ))
        .layer(Extension(Agent::Local)),
    );

    // when: counts are asked for without a project
    let response = server.get("/statistics/word-counts").expect_failure().await;

    // then: it is a bad request
    response.assert_status_bad_request();
}
