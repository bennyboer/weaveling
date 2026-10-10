use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_contract::PlaceDTO;
use appearances_core::{AppearanceCatalog, IdeaLink, PassageLink, Place, SectionLink, Subject};
use axum_test::TestServer;

fn an_idea(id: &str) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

#[tokio::test]
async fn an_idea_nobody_placed_answers_with_nothing() {
    let server = TestServer::new(appearances_rest::router(Arc::new(
        InMemoryAppearanceCatalog::new(),
    )));

    let response = server.get("/appearances?idea=idea_1").await;

    response.assert_status_ok();
    assert!(response.json::<Vec<PlaceDTO>>().is_empty());
}

#[tokio::test]
async fn an_idea_answers_with_every_place_it_appears() {
    let catalog = Arc::new(InMemoryAppearanceCatalog::new());
    for place in [
        Place::Section(SectionLink::from("section_1")),
        Place::Passage(PassageLink::from("passage_1")),
    ] {
        catalog
            .remember(&an_idea("idea_1"), &place, 1)
            .await
            .expect("remembering should succeed");
    }
    let server = TestServer::new(appearances_rest::router(catalog));

    let response = server.get("/appearances?idea=idea_1").await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<Vec<PlaceDTO>>(),
        vec![
            PlaceDTO::Passage {
                id: "passage_1".to_owned()
            },
            PlaceDTO::Section {
                id: "section_1".to_owned()
            },
        ]
    );
}

#[tokio::test]
async fn a_place_goes_on_the_wire_tagged_with_its_type() {
    let wire = serde_json::to_value(PlaceDTO::Section {
        id: "section_1".to_owned(),
    })
    .expect("a place is plain data");

    assert_eq!(
        wire,
        serde_json::json!({ "type": "section", "id": "section_1" }),
        "the client tells a section from a passage by this tag alone"
    );
}
