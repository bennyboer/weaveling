use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use eventsourcing::Agent;
use readings_contract::{ExtentDTO, ListedReadingCopyDTO, PartDTO, ReadingCopyDTO};

fn server() -> TestServer {
    TestServer::new(
        readings_rest::router(crate::allowing::everyone()).layer(Extension(Agent::Local)),
    )
}

#[tokio::test]
async fn a_project_s_copies_are_listed_newest_first() {
    // given: the readings
    let server = server();

    // when: a project's copies are asked for
    let listed = server
        .get("/readings/copies?project=project_loom")
        .await
        .json::<Vec<ListedReadingCopyDTO>>();

    // then: they come newest first
    let made: Vec<&str> = listed.iter().map(|copy| copy.made_at.as_str()).collect();
    let mut newest_first = made.clone();
    newest_first.sort_by(|one, other| other.cmp(one));
    assert_eq!(made, newest_first);
    // and: each says what it holds, who made it, and whether it is open
    assert_eq!(listed[0].extent, ExtentDTO::Book);
    assert!(listed[0].made_by_you);
    assert!(listed[0].open);
    assert!(!listed[1].made_by_you);
    assert!(!listed[1].open);
}

#[tokio::test]
async fn a_copy_answers_with_what_it_holds() {
    // given: a copy of a project
    let server = server();
    let listed = server
        .get("/readings/copies?project=project_loom")
        .await
        .json::<Vec<ListedReadingCopyDTO>>();

    // when: it is asked for
    let read = server
        .get(&format!("/readings/copies/{}", listed[0].id))
        .await
        .json::<ReadingCopyDTO>();

    // then: it is the copy listed, with its book
    assert_eq!(read.copy, listed[0]);
    assert_eq!(read.contents.project, "project_loom");
    assert!(matches!(read.contents.parts[0], PartDTO::Section { .. }));
}

#[tokio::test]
async fn a_copy_that_is_not_there_is_not_found() {
    // when: a copy nobody made is asked for
    let response = server().get("/readings/copies/copy_nobody_made").await;

    // then: it is not there
    response.assert_status(StatusCode::NOT_FOUND);
}
