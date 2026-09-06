use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachPieceRequest, MoveSectionRequest,
    OpenOutlineRequest, OutlineDTO, RetitleSectionRequest,
};
use time::{Duration, OffsetDateTime};

use crate::shapes::shaped;
use crate::wiring::wired;

const UNKNOWN_OUTLINE: &str = "outline_031VkO0hnpeQZUiAB7nDma";
const UNKNOWN_SECTION: &str = "section_031VkO0hnpeQZUiAB7nDma";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_server() -> TestServer {
    TestServer::new(wired(Arc::new(FixedClock::new(at(1_700_000_000)))).routes)
}

async fn an_open_outline(server: &TestServer) -> OutlineDTO {
    let response = server
        .post("/outlines")
        .json(&OpenOutlineRequest {
            project: "project_1".to_owned(),
        })
        .await;
    response.assert_status(StatusCode::OK);

    response.json()
}

async fn a_section(
    server: &TestServer,
    outline: &str,
    title: &str,
    under: Option<&str>,
    after: Option<&str>,
) -> AddedSectionResponse {
    let response = server
        .post(&format!("/outlines/{outline}/sections"))
        .json(&AddSectionRequest {
            under: under.map(str::to_owned),
            after: after.map(str::to_owned),
            title: title.to_owned(),
        })
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

fn shape(outline: &OutlineDTO) -> String {
    outline
        .sections
        .iter()
        .map(|placed| {
            let mut depth = 0;
            let mut walking = placed.parent.clone();

            while let Some(here) = walking {
                depth += 1;
                walking = outline
                    .sections
                    .iter()
                    .find(|other| other.section == here)
                    .and_then(|other| other.parent.clone());
            }

            format!("{}{}", "  ".repeat(depth), placed.title)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn opening_an_outline_comes_back_with_a_prefixed_id() {
    let server = a_server();

    let opened = an_open_outline(&server).await;

    assert!(
        opened.id.starts_with("outline_"),
        "an id should say what it is: {}",
        opened.id
    );
    assert_eq!(opened.project, "project_1");
    assert_eq!(opened.version, 1);
    assert!(
        opened.sections.is_empty(),
        "a book starts with no structure at all"
    );
}

#[tokio::test]
async fn opening_the_same_project_again_finds_the_outline_it_already_had() {
    let server = a_server();
    let first = an_open_outline(&server).await;

    let again = an_open_outline(&server).await;

    assert_eq!(again.id, first.id);
}

#[tokio::test]
async fn a_new_section_comes_back_named() {
    let server = a_server();
    let outline = an_open_outline(&server).await;

    let added = a_section(&server, &outline.id, "Part One", None, None).await;

    assert!(
        added.section.starts_with("section_"),
        "the caller cannot place anything under a section whose id it was never told"
    );
    assert_eq!(
        shape(&added.outline),
        shaped(
            "
            Part One
        "
        )
    );
}

#[tokio::test]
async fn sections_nest_where_they_are_put() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let part = a_section(&server, &outline.id, "Part One", None, None).await;
    let first = a_section(&server, &outline.id, "Chapter 1", Some(&part.section), None).await;

    let added = a_section(
        &server,
        &outline.id,
        "Chapter 2",
        Some(&part.section),
        Some(&first.section),
    )
    .await;

    assert_eq!(
        shape(&added.outline),
        shaped(
            "
            Part One
              Chapter 1
              Chapter 2
        "
        )
    );
}

#[tokio::test]
async fn a_section_can_be_retitled_for_the_table_of_contents() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let added = a_section(&server, &outline.id, "Chapter 1", None, None).await;

    let response = server
        .patch(&format!(
            "/outlines/{}/sections/{}",
            outline.id, added.section
        ))
        .json(&RetitleSectionRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineDTO>()),
        shaped(
            "
            The Silent Loom
        "
        )
    );
}

#[tokio::test]
async fn promoting_lifts_a_section_and_takes_what_followed_it_along() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    let arrival = a_section(
        &server,
        &outline.id,
        "Arrival",
        Some(&chapter.section),
        None,
    )
    .await;
    let rain = a_section(
        &server,
        &outline.id,
        "Rain",
        Some(&chapter.section),
        Some(&arrival.section),
    )
    .await;
    a_section(
        &server,
        &outline.id,
        "Storm",
        Some(&chapter.section),
        Some(&rain.section),
    )
    .await;

    let response = server
        .post(&format!(
            "/outlines/{}/sections/{}/promote",
            outline.id, rain.section
        ))
        .await;

    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineDTO>()),
        shaped(
            "
            Chapter 1
              Arrival
            Rain
              Storm
        "
        ),
        "the reading order must survive a promotion"
    );
}

#[tokio::test]
async fn demoting_nests_a_section_under_the_one_before_it() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let first = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    let second = a_section(
        &server,
        &outline.id,
        "Chapter 2",
        None,
        Some(&first.section),
    )
    .await;

    let response = server
        .post(&format!(
            "/outlines/{}/sections/{}/demote",
            outline.id, second.section
        ))
        .await;

    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineDTO>()),
        shaped(
            "
            Chapter 1
              Chapter 2
        "
        )
    );
}

#[tokio::test]
async fn a_section_can_be_moved_under_another_with_its_children() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let one = a_section(&server, &outline.id, "Part One", None, None).await;
    let two = a_section(&server, &outline.id, "Part Two", None, Some(&one.section)).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", Some(&one.section), None).await;
    a_section(
        &server,
        &outline.id,
        "Arrival",
        Some(&chapter.section),
        None,
    )
    .await;

    let response = server
        .put(&format!(
            "/outlines/{}/sections/{}/place",
            outline.id, chapter.section
        ))
        .json(&MoveSectionRequest {
            under: Some(two.section.clone()),
            after: None,
        })
        .await;

    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineDTO>()),
        shaped(
            "
            Part One
            Part Two
              Chapter 1
                Arrival
        "
        )
    );
}

#[tokio::test]
async fn a_section_cannot_be_moved_inside_itself() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let part = a_section(&server, &outline.id, "Part One", None, None).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", Some(&part.section), None).await;

    let response = server
        .put(&format!(
            "/outlines/{}/sections/{}/place",
            outline.id, part.section
        ))
        .json(&MoveSectionRequest {
            under: Some(chapter.section.clone()),
            after: None,
        })
        .await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn removing_a_section_lifts_its_children_into_its_place() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let part = a_section(&server, &outline.id, "Part One", None, None).await;
    a_section(&server, &outline.id, "Chapter 1", Some(&part.section), None).await;
    a_section(&server, &outline.id, "Part Two", None, Some(&part.section)).await;

    let response = server
        .delete(&format!(
            "/outlines/{}/sections/{}",
            outline.id, part.section
        ))
        .await;

    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineDTO>()),
        shaped(
            "
            Chapter 1
            Part Two
        "
        ),
        "pruning a chapter must not take the scenes inside it with it"
    );
}

#[tokio::test]
async fn a_piece_is_attached_to_a_section_and_read_back_in_order() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", None, None).await;

    for (piece, after) in [("piece_1", None), ("piece_2", Some("piece_1"))] {
        let response = server
            .post(&format!("/outlines/{}/pieces", outline.id))
            .json(&AttachPieceRequest {
                piece: piece.to_owned(),
                section: chapter.section.clone(),
                after: after.map(str::to_owned),
            })
            .await;
        response.assert_status(StatusCode::OK);
    }

    let found: OutlineDTO = server
        .get(&format!("/outlines/{}", outline.id))
        .await
        .json();

    assert_eq!(found.sections[0].pieces, vec!["piece_1", "piece_2"]);
}

#[tokio::test]
async fn attaching_a_piece_that_sits_elsewhere_moves_it() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let one = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    let two = a_section(&server, &outline.id, "Chapter 2", None, Some(&one.section)).await;

    for section in [&one.section, &two.section] {
        server
            .post(&format!("/outlines/{}/pieces", outline.id))
            .json(&AttachPieceRequest {
                piece: "piece_1".to_owned(),
                section: section.clone(),
                after: None,
            })
            .await
            .assert_status(StatusCode::OK);
    }

    let found: OutlineDTO = server
        .get(&format!("/outlines/{}", outline.id))
        .await
        .json();

    assert!(found.sections[0].pieces.is_empty());
    assert_eq!(found.sections[1].pieces, vec!["piece_1"]);
}

#[tokio::test]
async fn detaching_a_piece_takes_it_out_of_the_book() {
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    server
        .post(&format!("/outlines/{}/pieces", outline.id))
        .json(&AttachPieceRequest {
            piece: "piece_1".to_owned(),
            section: chapter.section.clone(),
            after: None,
        })
        .await
        .assert_status(StatusCode::OK);

    server
        .delete(&format!("/outlines/{}/pieces/piece_1", outline.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let found: OutlineDTO = server
        .get(&format!("/outlines/{}", outline.id))
        .await
        .json();

    assert!(found.sections[0].pieces.is_empty());
}

#[tokio::test]
async fn a_section_with_no_pieces_is_accepted_because_planning_comes_first() {
    let server = a_server();
    let outline = an_open_outline(&server).await;

    let added = a_section(&server, &outline.id, "Chapter 9", None, None).await;

    assert!(
        added.outline.sections[0].pieces.is_empty(),
        "an empty leaf is a hole the view flags, never a refusal the domain makes"
    );
}

#[tokio::test]
async fn a_section_can_be_added_without_naming_it_at_all() {
    let server = a_server();
    let outline = an_open_outline(&server).await;

    let response = server
        .post(&format!("/outlines/{}/sections", outline.id))
        .json(&serde_json::json!({}))
        .await;

    response.assert_status(StatusCode::CREATED);
    assert_eq!(
        response.json::<AddedSectionResponse>().outline.sections[0].title,
        "",
        "a title left out of the request is a section waiting to be named, not a bad request"
    );
}

#[tokio::test]
async fn an_unnamed_section_is_stored_as_empty_rather_than_as_untitled() {
    let server = a_server();
    let outline = an_open_outline(&server).await;

    let added = a_section(&server, &outline.id, "   ", None, None).await;

    assert_eq!(
        added.outline.sections[0].title, "",
        "'Untitled' is a rendering, never a value"
    );
}

#[tokio::test]
async fn an_outline_nobody_started_is_not_found() {
    let server = a_server();

    server
        .get(&format!("/outlines/{UNKNOWN_OUTLINE}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_section_nobody_added_is_not_found() {
    let server = a_server();
    let outline = an_open_outline(&server).await;

    server
        .post(&format!(
            "/outlines/{}/sections/{UNKNOWN_SECTION}/promote",
            outline.id
        ))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_address_that_is_not_an_id_is_refused_before_anything_is_looked_up() {
    let server = a_server();

    server
        .get("/outlines/not-an-outline")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn every_change_moves_the_version_the_etag_reports() {
    let server = a_server();
    let outline = an_open_outline(&server).await;

    let added = a_section(&server, &outline.id, "Part One", None, None).await;

    assert!(
        added.outline.version > outline.version,
        "a client holding a stale version must be able to tell"
    );
}
