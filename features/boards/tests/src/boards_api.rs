use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum_test::{TestResponse, TestServer};
use boards_contract::{
    BoardDTO, OpenBoardRequest, PinIdeaRequest, PositionedIdeaDTO, ReshapeIdeaRequest, SizeDTO,
    SpotDTO, refused,
};
use boards_core::BoardsCore;
use clock::FixedClock;
use eventsourcing::Agent;
use serde_json::Value;

use crate::wiring;

const UNKNOWN_BOARD: &str = "board_031VkO0hnpeQZUiAB7nDma";

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
    assert_eq!(told["status"], status.as_u16(), "{told}");

    told
}

fn a_server() -> TestServer {
    a_server_and_its_boards().0
}

fn a_server_and_its_boards() -> (TestServer, BoardsCore) {
    let wired = wiring::wired(Arc::new(FixedClock::new(clock::at(1_700_000_000))));

    (
        TestServer::new(wired.routes.layer(Extension(Agent::Local))),
        wired.boards,
    )
}

fn opening(project: &str) -> OpenBoardRequest {
    OpenBoardRequest {
        project: project.to_owned(),
    }
}

fn a_spot(x: i64, y: i64) -> SpotDTO {
    SpotDTO { x, y }
}

fn a_size() -> SizeDTO {
    SizeDTO {
        width: 168,
        height: 84,
    }
}

async fn an_open_board(server: &TestServer) -> BoardDTO {
    let response = server.post("/boards").json(&opening("project_1")).await;
    response.assert_status(StatusCode::OK);

    response.json()
}

async fn a_pinned_idea(server: &TestServer, board: &BoardDTO, idea: &str, at: SpotDTO) -> BoardDTO {
    let response = server
        .post(&format!("/boards/{}/ideas", board.id))
        .json(&PinIdeaRequest {
            idea: idea.to_owned(),
            spot: at,
            size: a_size(),
        })
        .await;
    response.assert_status(StatusCode::OK);

    response.json()
}

#[tokio::test]
async fn opening_a_board_comes_back_with_a_prefixed_id() {
    // given: a server
    let server = a_server();

    // when: a board is opened
    let opened = an_open_board(&server).await;

    // then: its id says it is a board
    assert!(
        opened.id.starts_with("board_"),
        "an id should say what it is: {}",
        opened.id
    );
    // and: it is the project's first and empty board
    assert_eq!(opened.project, "project_1");
    assert_eq!(opened.version, 1);
    assert!(opened.ideas.is_empty());
}

#[tokio::test]
async fn opening_a_board_twice_returns_the_same_board() {
    // given: an open board
    let server = a_server();
    let first = an_open_board(&server).await;

    // when: it is opened again
    let second = an_open_board(&server).await;

    // then: it is the same board, unchanged
    assert_eq!(
        first.id, second.id,
        "posting to open is find-or-start, so it is safe to repeat"
    );
    assert_eq!(second.version, 1, "nothing further happened to it");
}

#[tokio::test]
async fn a_board_carries_an_etag_of_its_version() {
    // given: a server
    let server = a_server();

    // when: a board is opened
    let response = server.post("/boards").json(&opening("project_1")).await;

    // then: it carries its version as an etag
    assert_eq!(response.header("etag"), "\"1\"");
}

#[tokio::test]
async fn a_board_can_be_read_back_by_its_id() {
    // given: an open board
    let server = a_server();
    let opened = an_open_board(&server).await;

    // when: it is read by its id
    let response = server.get(&format!("/boards/{}", opened.id)).await;
    response.assert_status(StatusCode::OK);

    // then: it is the board that was opened
    assert_eq!(response.json::<BoardDTO>(), opened);
}

#[tokio::test]
async fn a_board_nobody_opened_is_not_found() {
    // given: no board by that id
    let server = a_server();

    // then: reading it is not found
    refused_as(
        server.get(&format!("/boards/{UNKNOWN_BOARD}")).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_id_of_another_kind_is_a_bad_request() {
    // given: a server
    let server = a_server();

    // then: reading an id of another kind is a bad request
    refused_as(
        server.get("/boards/idea_031VkO0hnpeQZUiAB7nDma").await,
        StatusCode::BAD_REQUEST,
        refused::ID_INVALID,
    );
}

#[tokio::test]
async fn a_pinned_idea_comes_back_where_it_was_dropped() {
    // given: an open board
    let server = a_server();
    let opened = an_open_board(&server).await;

    // when: an idea is pinned at a spot
    let pinned = a_pinned_idea(&server, &opened, "idea_1", a_spot(120, -40)).await;

    // then: the board holds it at that spot
    assert_eq!(
        pinned.ideas,
        vec![PositionedIdeaDTO {
            idea: "idea_1".to_owned(),
            spot: a_spot(120, -40),
            size: a_size(),
        }]
    );
    assert_eq!(pinned.version, 2);
}

#[tokio::test]
async fn pinning_the_same_idea_twice_is_a_conflict() {
    // given: a board holding an idea
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(0, 0)).await;

    // then: pinning it again is a conflict
    refused_as(
        server
            .post(&format!("/boards/{}/ideas", opened.id))
            .json(&PinIdeaRequest {
                idea: "idea_1".to_owned(),
                spot: a_spot(9, 9),
                size: a_size(),
            })
            .await,
        StatusCode::CONFLICT,
        refused::ALREADY_PINNED,
    );
}

#[tokio::test]
async fn a_moved_idea_comes_back_at_its_new_spot() {
    // given: a board holding an idea
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(10, 10)).await;

    // when: the idea is moved
    let response = server
        .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
        .json(&ReshapeIdeaRequest {
            spot: Some(a_spot(300, 20)),
            size: None,
        })
        .await;
    response.assert_status(StatusCode::OK);

    // then: the board holds it at its new spot
    assert_eq!(
        response.json::<BoardDTO>().ideas,
        vec![PositionedIdeaDTO {
            idea: "idea_1".to_owned(),
            spot: a_spot(300, 20),
            size: a_size(),
        }]
    );
}

#[tokio::test]
async fn moving_an_idea_that_is_not_on_the_board_is_not_found() {
    // given: an open board without the idea
    let server = a_server();
    let opened = an_open_board(&server).await;

    // then: moving the idea is not found
    refused_as(
        server
            .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
            .json(&ReshapeIdeaRequest {
                spot: Some(a_spot(1, 1)),
                size: None,
            })
            .await,
        StatusCode::NOT_FOUND,
        refused::NOT_PINNED,
    );
}

#[tokio::test]
async fn unpinning_an_idea_that_is_not_on_the_board_is_not_found() {
    // given: an open board without the idea
    let server = a_server();
    let opened = an_open_board(&server).await;

    // then: unpinning the idea is not found
    refused_as(
        server
            .delete(&format!("/boards/{}/ideas/idea_1", opened.id))
            .await,
        StatusCode::NOT_FOUND,
        refused::NOT_PINNED,
    );
}

#[tokio::test]
async fn an_unpinned_idea_leaves_the_board() {
    // given: a board holding an idea
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(10, 10)).await;

    // when: the idea is unpinned
    server
        .delete(&format!("/boards/{}/ideas/idea_1", opened.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: the board read again holds nothing
    let response = server.get(&format!("/boards/{}", opened.id)).await;
    assert!(response.json::<BoardDTO>().ideas.is_empty());
}

#[tokio::test]
async fn a_move_against_a_stale_version_is_refused() {
    // given: a board that moved past its first version
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(10, 10)).await;

    // then: a move against the first version is refused as outdated
    refused_as(
        server
            .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
            .add_header("if-match", "\"1\"")
            .json(&ReshapeIdeaRequest {
                spot: Some(a_spot(20, 20)),
                size: None,
            })
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );
}

#[tokio::test]
async fn a_move_against_the_current_version_is_allowed() {
    // given: a board holding an idea
    let server = a_server();
    let opened = an_open_board(&server).await;
    let pinned = a_pinned_idea(&server, &opened, "idea_1", a_spot(10, 10)).await;

    // then: a move against its current version is accepted
    server
        .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
        .add_header("if-match", format!("\"{}\"", pinned.version))
        .json(&ReshapeIdeaRequest {
            spot: Some(a_spot(20, 20)),
            size: None,
        })
        .await
        .assert_status(StatusCode::OK);
}

#[tokio::test]
async fn an_if_match_that_is_not_a_version_is_a_bad_request() {
    // given: an open board
    let server = a_server();
    let opened = an_open_board(&server).await;

    // then: an if-match that is not a version is a bad request
    refused_as(
        server
            .delete(&format!("/boards/{}/ideas/idea_1", opened.id))
            .add_header("if-match", "\"not-a-version\"")
            .await,
        StatusCode::BAD_REQUEST,
        "unreadable",
    );
}

#[tokio::test]
async fn boards_of_other_projects_are_left_alone() {
    // given: an open board
    let server = a_server();
    let mine = an_open_board(&server).await;

    // when: another project's board is opened
    let response = server.post("/boards").json(&opening("project_2")).await;
    response.assert_status(StatusCode::OK);

    // then: it is a different board
    assert_ne!(response.json::<BoardDTO>().id, mine.id);
}

#[tokio::test]
async fn a_resized_idea_comes_back_at_its_new_extent() {
    // given: a board holding an idea
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(10, 10)).await;

    // when: the idea is resized
    let response = server
        .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
        .json(&ReshapeIdeaRequest {
            spot: None,
            size: Some(SizeDTO {
                width: 400,
                height: 90,
            }),
        })
        .await;
    response.assert_status(StatusCode::OK);

    // then: the board holds it at its new extent
    assert_eq!(
        response.json::<BoardDTO>().ideas,
        vec![PositionedIdeaDTO {
            idea: "idea_1".to_owned(),
            spot: a_spot(10, 10),
            size: SizeDTO {
                width: 400,
                height: 90,
            },
        }]
    );
}

#[tokio::test]
async fn dragging_an_edge_moves_and_resizes_in_one_request() {
    // given: a board holding an idea
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(100, 100)).await;

    // when: the idea is moved and resized in one request
    let response = server
        .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
        .json(&ReshapeIdeaRequest {
            spot: Some(a_spot(60, 100)),
            size: Some(SizeDTO {
                width: 208,
                height: 84,
            }),
        })
        .await;
    response.assert_status(StatusCode::OK);

    // then: the board holds it at its new spot and extent
    let board = response.json::<BoardDTO>();
    assert_eq!(
        board.ideas,
        vec![PositionedIdeaDTO {
            idea: "idea_1".to_owned(),
            spot: a_spot(60, 100),
            size: SizeDTO {
                width: 208,
                height: 84,
            },
        }]
    );
    // and: both changes were recorded
    assert_eq!(
        board.version, 4,
        "one gesture, but the log holds both what moved and what stretched"
    );
}

#[tokio::test]
async fn a_card_cannot_be_resized_into_nothing() {
    // given: a board holding an idea
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(10, 10)).await;

    // then: resizing it to no width is refused as shapeless
    refused_as(
        server
            .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
            .json(&ReshapeIdeaRequest {
                spot: None,
                size: Some(SizeDTO {
                    width: 0,
                    height: 84,
                }),
            })
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::SHAPELESS,
    );
}

#[tokio::test]
async fn a_moved_idea_comes_back_at_the_front_of_the_stack() {
    // given: a board holding two ideas, one over the other
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_idea(&server, &opened, "idea_1", a_spot(10, 10)).await;
    a_pinned_idea(&server, &opened, "idea_2", a_spot(20, 20)).await;

    // when: the one underneath is moved
    let response = server
        .patch(&format!("/boards/{}/ideas/idea_1", opened.id))
        .json(&ReshapeIdeaRequest {
            spot: Some(a_spot(300, 20)),
            size: None,
        })
        .await;
    response.assert_status(StatusCode::OK);

    // then: it is now on top
    let order: Vec<String> = response
        .json::<BoardDTO>()
        .ideas
        .into_iter()
        .map(|held| held.idea)
        .collect();
    assert_eq!(
        order,
        vec!["idea_2".to_owned(), "idea_1".to_owned()],
        "the board lists its ideas bottom to top, and the moved one is now on top"
    );
}

#[tokio::test]
async fn a_board_discarded_with_its_project_refuses_what_comes_after() {
    // given: a board discarded with its project
    let (server, boards) = a_server_and_its_boards();
    let opened = an_open_board(&server).await;
    boards
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");

    // when: an idea is pinned to it
    let response = server
        .post(&format!("/boards/{}/ideas", opened.id))
        .json(&PinIdeaRequest {
            idea: "idea_1".to_owned(),
            spot: a_spot(0, 0),
            size: a_size(),
        })
        .await;

    // then: it is refused as discarded
    refused_as(response, StatusCode::CONFLICT, refused::DISCARDED);
}
