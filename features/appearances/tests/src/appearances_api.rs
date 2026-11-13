use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_contract::PlaceDTO;
use appearances_core::{IdeaLink, Place, ProjectLink, SceneLink, SectionLink, Subject};
use axum::Extension;
use axum_test::TestServer;
use eventsourcing::Agent;
use transacting::InMemoryTransactions;

use crate::committed::Committed;

fn an_idea(id: &str) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

fn served(catalog: Arc<InMemoryAppearanceCatalog>) -> TestServer {
    TestServer::new(
        appearances_rest::router(crate::allowing::everyone(
            Arc::new(InMemoryTransactions::new()),
            catalog,
        ))
        .layer(Extension(Agent::Local)),
    )
}

#[tokio::test]
async fn an_idea_nobody_placed_answers_with_nothing() {
    // given: nothing placed anywhere
    let server = served(Arc::new(InMemoryAppearanceCatalog::new()));

    // when: where an idea appears is asked
    let response = server.get("/appearances?idea=idea_1").await;

    // then: nothing is answered
    response.assert_status_ok();
    assert!(response.json::<Vec<PlaceDTO>>().is_empty());
}

#[tokio::test]
async fn an_idea_answers_with_every_place_it_appears() {
    // given: an idea of a project, placed in a section and in a scene of it
    let catalog = Arc::new(InMemoryAppearanceCatalog::new());
    catalog
        .remember_project_committed(&an_idea("idea_1"), &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");
    for place in [
        Place::Section(SectionLink::from("section_1")),
        Place::Scene(SceneLink::from("scene_1")),
    ] {
        catalog
            .remember_committed(&an_idea("idea_1"), &place, 1)
            .await
            .expect("remembering should succeed");
    }
    let server = served(catalog);

    // when: where it appears is asked
    let response = server.get("/appearances?idea=idea_1").await;

    // then: both places are answered
    response.assert_status_ok();
    assert_eq!(
        response.json::<Vec<PlaceDTO>>(),
        vec![
            PlaceDTO::Scene {
                id: "scene_1".to_owned()
            },
            PlaceDTO::Section {
                id: "section_1".to_owned()
            },
        ]
    );
}

#[tokio::test]
async fn a_place_goes_on_the_wire_tagged_with_its_type() {
    // when: a section goes on the wire
    let wire = serde_json::to_value(PlaceDTO::Section {
        id: "section_1".to_owned(),
    })
    .expect("a place is plain data");

    // then: it is tagged with its type
    assert_eq!(
        wire,
        serde_json::json!({ "type": "section", "id": "section_1" }),
        "the client tells a section from a scene by this tag alone"
    );
}
