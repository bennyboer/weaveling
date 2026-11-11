use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::Agent;
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachRequest, AttachmentDTO, MoveSectionRequest,
    OpenOutlineRequest, OutlineChangeDTO, OutlineDTO, RetitleSectionRequest, refused,
};
use outline_core::{OutlinesCore, SectionTitle};
use serde_json::Value;

use crate::shapes;
use crate::wiring;

const UNKNOWN_OUTLINE: &str = "outline_031VkO0hnpeQZUiAB7nDma";
const UNKNOWN_SECTION: &str = "section_031VkO0hnpeQZUiAB7nDma";

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
    assert_eq!(told["status"], status.as_u16(), "{told}");

    told
}

fn a_server() -> TestServer {
    a_server_and_its_outlines().0
}

fn a_server_and_its_outlines() -> (TestServer, OutlinesCore) {
    let wired = wiring::wired(Arc::new(FixedClock::new(clock::at(1_700_000_000))));

    (
        TestServer::new(wired.routes.layer(Extension(Agent::Local))),
        wired.outlines,
    )
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

fn shape(outline: &OutlineChangeDTO) -> String {
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
    // given: a project with no outline
    let server = a_server();

    // when: its outline is opened
    let opened = an_open_outline(&server).await;

    // then: it comes back with an id saying it is an outline
    assert!(
        opened.id.starts_with("outline_"),
        "an id should say what it is: {}",
        opened.id
    );
    // and: of its project, at its first version, with no sections
    assert_eq!(opened.project, "project_1");
    assert_eq!(opened.version, 1);
    assert!(
        opened.sections.is_empty(),
        "a book starts with no structure at all"
    );
}

#[tokio::test]
async fn opening_the_same_project_again_finds_the_outline_it_already_had() {
    // given: an open outline
    let server = a_server();
    let first = an_open_outline(&server).await;

    // when: the same project's outline is opened again
    let again = an_open_outline(&server).await;

    // then: it is the same outline
    assert_eq!(again.id, first.id);
}

#[tokio::test]
async fn a_new_section_comes_back_named() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a section is added
    let added = a_section(&server, &outline.id, "Part One", None, None).await;

    // then: it comes back with an id saying it is a section
    assert!(
        added.section.starts_with("section_"),
        "the caller cannot place anything under a section whose id it was never told"
    );
    // and: the outline holds it
    assert_eq!(
        shape(&added.outline),
        shapes::shaped(
            "
            Part One
        "
        )
    );
}

#[tokio::test]
async fn sections_nest_where_they_are_put() {
    // given: an outline with a part holding a chapter
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let part = a_section(&server, &outline.id, "Part One", None, None).await;
    let first = a_section(&server, &outline.id, "Chapter 1", Some(&part.section), None).await;

    // when: a section is added under the part after the chapter
    let added = a_section(
        &server,
        &outline.id,
        "Chapter 2",
        Some(&part.section),
        Some(&first.section),
    )
    .await;

    // then: it nests there
    assert_eq!(
        shape(&added.outline),
        shapes::shaped(
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
    // given: an outline with a chapter
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let added = a_section(&server, &outline.id, "Chapter 1", None, None).await;

    // when: the chapter is retitled
    let response = server
        .patch(&format!(
            "/outlines/{}/sections/{}",
            outline.id, added.section
        ))
        .json(&RetitleSectionRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    // then: the outline holds it under its new title
    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineChangeDTO>()),
        shapes::shaped(
            "
            The Silent Loom
        "
        )
    );
}

#[tokio::test]
async fn promoting_lifts_a_section_and_takes_what_followed_it_along() {
    // given: a chapter with three sections in it
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

    // when: the middle section is promoted
    let response = server
        .post(&format!(
            "/outlines/{}/sections/{}/promote",
            outline.id, rain.section
        ))
        .await;

    // then: it sits after the chapter, with the section after it as its child
    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineChangeDTO>()),
        shapes::shaped(
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
    // given: two chapters
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

    // when: the second is demoted
    let response = server
        .post(&format!(
            "/outlines/{}/sections/{}/demote",
            outline.id, second.section
        ))
        .await;

    // then: it nests under the first
    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineChangeDTO>()),
        shapes::shaped(
            "
            Chapter 1
              Chapter 2
        "
        )
    );
}

#[tokio::test]
async fn a_section_can_be_moved_under_another_with_its_children() {
    // given: two parts, the first with a chapter holding a section
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

    // when: the chapter is moved under the second part
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

    // then: its section goes along
    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineChangeDTO>()),
        shapes::shaped(
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
    // given: a part with a chapter in it
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let part = a_section(&server, &outline.id, "Part One", None, None).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", Some(&part.section), None).await;

    // when: the part is moved under the chapter
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

    // then: it is refused as it would contain itself
    refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::WOULD_CONTAIN_ITSELF,
    );
}

#[tokio::test]
async fn removing_a_section_lifts_its_children_into_its_place() {
    // given: a part with a chapter in it, and a part after it
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let part = a_section(&server, &outline.id, "Part One", None, None).await;
    a_section(&server, &outline.id, "Chapter 1", Some(&part.section), None).await;
    a_section(&server, &outline.id, "Part Two", None, Some(&part.section)).await;

    // when: the first part is removed
    let response = server
        .delete(&format!(
            "/outlines/{}/sections/{}",
            outline.id, part.section
        ))
        .await;

    // then: its chapter takes its place
    response.assert_status(StatusCode::OK);
    assert_eq!(
        shape(&response.json::<OutlineChangeDTO>()),
        shapes::shaped(
            "
            Chapter 1
            Part Two
        "
        ),
        "pruning a chapter must not take the scenes inside it with it"
    );
}

fn a_scene(id: &str) -> AttachmentDTO {
    AttachmentDTO::Scene { id: id.to_owned() }
}

#[tokio::test]
async fn a_scene_is_attached_to_a_section_and_read_back_in_order() {
    // given: an outline with a chapter
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", None, None).await;

    // when: two scenes are attached to the chapter, one after the other
    for (scene, after) in [("scene_1", None), ("scene_2", Some("scene_1"))] {
        let response = server
            .post(&format!("/outlines/{}/attachments", outline.id))
            .json(&AttachRequest {
                attachment: a_scene(scene),
                section: chapter.section.clone(),
                after: after.map(a_scene),
            })
            .await;
        response.assert_status(StatusCode::OK);
    }

    // and: the outline is read
    let found: OutlineDTO = server
        .get(&format!("/outlines/{}", outline.id))
        .await
        .json();

    // then: the chapter holds them in that order
    assert_eq!(
        found.sections[0].attachments,
        vec![a_scene("scene_1"), a_scene("scene_2")]
    );
}

#[tokio::test]
async fn attaching_a_scene_that_sits_elsewhere_moves_it() {
    // given: an outline with two chapters
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let one = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    let two = a_section(&server, &outline.id, "Chapter 2", None, Some(&one.section)).await;

    // when: a scene is attached to the first and then to the second
    for section in [&one.section, &two.section] {
        server
            .post(&format!("/outlines/{}/attachments", outline.id))
            .json(&AttachRequest {
                attachment: AttachmentDTO::Scene {
                    id: "scene_1".to_owned(),
                },
                section: section.clone(),
                after: None,
            })
            .await
            .assert_status(StatusCode::OK);
    }

    // and: the outline is read
    let found: OutlineDTO = server
        .get(&format!("/outlines/{}", outline.id))
        .await
        .json();

    // then: the first holds nothing
    assert!(found.sections[0].attachments.is_empty());
    // and: the second holds the scene
    assert_eq!(found.sections[1].attachments, vec![a_scene("scene_1")]);
}

#[tokio::test]
async fn detaching_a_scene_takes_it_out_of_the_book() {
    // given: a chapter holding a scene
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    server
        .post(&format!("/outlines/{}/attachments", outline.id))
        .json(&AttachRequest {
            attachment: AttachmentDTO::Scene {
                id: "scene_1".to_owned(),
            },
            section: chapter.section.clone(),
            after: None,
        })
        .await
        .assert_status(StatusCode::OK);

    // when: the scene is detached
    server
        .delete(&format!(
            "/outlines/{}/attachments/scenes/scene_1",
            outline.id
        ))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // and: the outline is read
    let found: OutlineDTO = server
        .get(&format!("/outlines/{}", outline.id))
        .await
        .json();

    // then: the chapter holds nothing
    assert!(found.sections[0].attachments.is_empty());
}

#[tokio::test]
async fn a_section_with_no_scenes_is_accepted_because_planning_comes_first() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a section is added
    a_section(&server, &outline.id, "Chapter 9", None, None).await;

    // and: the outline is read
    let found: OutlineDTO = server
        .get(&format!("/outlines/{}", outline.id))
        .await
        .json();
    // then: the section holds nothing
    assert!(
        found.sections[0].attachments.is_empty(),
        "an empty leaf is a hole the view flags, never a refusal the domain makes"
    );
}

#[tokio::test]
async fn a_section_can_be_added_without_naming_it_at_all() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a section is added with no title given
    let response = server
        .post(&format!("/outlines/{}/sections", outline.id))
        .json(&serde_json::json!({}))
        .await;

    // then: it is added unnamed
    response.assert_status(StatusCode::CREATED);
    assert_eq!(
        response.json::<AddedSectionResponse>().outline.sections[0].title,
        "",
        "a title left out of the request is a section waiting to be named, not a bad request"
    );
}

#[tokio::test]
async fn an_unnamed_section_is_stored_as_empty_rather_than_as_untitled() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a section titled only in blanks is added
    let added = a_section(&server, &outline.id, "   ", None, None).await;

    // then: its title is empty
    assert_eq!(
        added.outline.sections[0].title, "",
        "'Untitled' is a rendering, never a value"
    );
}

#[tokio::test]
async fn an_outline_nobody_started_is_not_found() {
    // given: no outline started
    let server = a_server();

    // then: reading one is refused as not found
    refused_as(
        server.get(&format!("/outlines/{UNKNOWN_OUTLINE}")).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn a_section_nobody_added_is_not_found() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // then: promoting a section nobody added is refused as no such section
    refused_as(
        server
            .post(&format!(
                "/outlines/{}/sections/{UNKNOWN_SECTION}/promote",
                outline.id
            ))
            .await,
        StatusCode::NOT_FOUND,
        refused::NO_SUCH_SECTION,
    );
}

#[tokio::test]
async fn an_address_that_is_not_an_id_is_refused_before_anything_is_looked_up() {
    // given: no outline started
    let server = a_server();

    // then: reading at an address that is not an id is refused as invalid
    refused_as(
        server.get("/outlines/not-an-outline").await,
        StatusCode::BAD_REQUEST,
        refused::ID_INVALID,
    );
}

#[tokio::test]
async fn every_change_moves_the_version_the_etag_reports() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a section is added
    let added = a_section(&server, &outline.id, "Part One", None, None).await;

    // then: the outline's version moves on
    assert!(
        added.outline.version > outline.version,
        "a client holding a stale version must be able to tell"
    );
}

async fn attaching(
    server: &TestServer,
    outline: &str,
    scene: &str,
    section: &str,
) -> OutlineChangeDTO {
    let response = server
        .post(&format!("/outlines/{outline}/attachments"))
        .json(&AttachRequest {
            attachment: a_scene(scene),
            section: section.to_owned(),
            after: None,
        })
        .await;
    response.assert_status(StatusCode::OK);

    response.json()
}

fn attachments_sent(change: &OutlineChangeDTO) -> Vec<(String, Option<Vec<AttachmentDTO>>)> {
    change
        .sections
        .iter()
        .map(|placed| (placed.title.clone(), placed.attachments.clone()))
        .collect()
}

#[tokio::test]
async fn an_attach_answers_with_the_scenes_of_its_own_section_only() {
    // given: two chapters, the first holding a scene
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let one = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    let two = a_section(&server, &outline.id, "Chapter 2", None, Some(&one.section)).await;
    attaching(&server, &outline.id, "scene_1", &one.section).await;

    // when: a scene is attached to the second
    let change = attaching(&server, &outline.id, "scene_2", &two.section).await;

    // then: the answer holds the scenes of the second only
    assert_eq!(
        attachments_sent(&change),
        vec![
            ("Chapter 1".to_owned(), None),
            ("Chapter 2".to_owned(), Some(vec![a_scene("scene_2")])),
        ],
        "a book of thousands of scenes must not travel whole for every scene placed"
    );
}

#[tokio::test]
async fn a_scene_moved_between_sections_answers_with_both() {
    // given: two chapters, the first holding a scene
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let one = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    let two = a_section(&server, &outline.id, "Chapter 2", None, Some(&one.section)).await;
    attaching(&server, &outline.id, "scene_1", &one.section).await;

    // when: the scene is attached to the second
    let change = attaching(&server, &outline.id, "scene_1", &two.section).await;

    // then: the answer holds the scenes of both
    assert_eq!(
        attachments_sent(&change),
        vec![
            ("Chapter 1".to_owned(), Some(Vec::new())),
            ("Chapter 2".to_owned(), Some(vec![a_scene("scene_1")])),
        ],
        "the section it left changed as much as the one it went to"
    );
}

#[tokio::test]
async fn a_change_answers_from_which_version_it_was_decided() {
    // given: a chapter holding a scene
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let one = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    let first = attaching(&server, &outline.id, "scene_1", &one.section).await;

    // when: another scene is attached to it
    let second = attaching(&server, &outline.id, "scene_2", &one.section).await;

    // then: each answer is decided from the version before it
    assert_eq!(first.since + 1, first.version);
    assert_eq!(
        second.since, first.version,
        "a client holding another version than the one decided from cannot trust what a change leaves out"
    );
    assert_eq!(second.version, first.version + 1);
}

#[tokio::test]
async fn a_retitle_answers_with_the_shape_of_the_book_and_no_scenes() {
    // given: a chapter holding a scene
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let one = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    attaching(&server, &outline.id, "scene_1", &one.section).await;

    // when: the chapter is retitled
    let change: OutlineChangeDTO = server
        .patch(&format!(
            "/outlines/{}/sections/{}",
            outline.id, one.section
        ))
        .json(&RetitleSectionRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .json();

    // then: the answer holds the sections and no scenes
    assert_eq!(
        attachments_sent(&change),
        vec![("The Silent Loom".to_owned(), None)]
    );
}

#[tokio::test]
async fn a_section_title_too_long_is_refused_telling_how_long_it_may_be() {
    // given: an outline with a chapter
    let server = a_server();
    let outline = an_open_outline(&server).await;
    let added = a_section(&server, &outline.id, "Chapter 1", None, None).await;

    // when: the chapter is retitled past the longest a title may be
    let response = server
        .patch(&format!(
            "/outlines/{}/sections/{}",
            outline.id, added.section
        ))
        .json(&RetitleSectionRequest {
            title: "a".repeat(SectionTitle::MAX_CHARS + 1),
        })
        .await;

    // then: it is refused as too long
    let told = refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TITLE_TOO_LONG,
    );
    // and: the answer tells how long it may be and how long it was
    assert_eq!(told["max"], SectionTitle::MAX_CHARS);
    assert_eq!(told["actual"], SectionTitle::MAX_CHARS + 1);
}

#[tokio::test]
async fn a_section_title_with_a_control_character_is_refused_as_such() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a section titled with a control character is added
    let response = server
        .post(&format!("/outlines/{}/sections", outline.id))
        .json(&AddSectionRequest {
            under: None,
            after: None,
            title: "Chapter\u{7} 1".to_owned(),
        })
        .await;

    // then: it is refused as such
    refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TITLE_CONTROL_CHARACTER,
    );
}

#[tokio::test]
async fn a_section_placed_after_one_that_is_not_beside_it_is_refused() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a section is added after one that is not there
    let response = server
        .post(&format!("/outlines/{}/sections", outline.id))
        .json(&AddSectionRequest {
            under: None,
            after: Some(UNKNOWN_SECTION.to_owned()),
            title: "Chapter 1".to_owned(),
        })
        .await;

    // then: it is refused as no such neighbour
    refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::NO_SUCH_NEIGHBOUR,
    );
}

#[tokio::test]
async fn detaching_what_is_not_in_the_outline_is_refused() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a scene not in it is detached
    let response = server
        .delete(&format!(
            "/outlines/{}/attachments/scenes/scene_1",
            outline.id
        ))
        .await;

    // then: it is refused as not attached
    refused_as(response, StatusCode::NOT_FOUND, refused::NOT_ATTACHED);
}

#[tokio::test]
async fn an_outline_holds_no_other_kind_of_thing() {
    // given: an open outline
    let server = a_server();
    let outline = an_open_outline(&server).await;

    // when: a thing of no kind it holds is detached
    let response = server
        .delete(&format!(
            "/outlines/{}/attachments/tapestries/tapestry_1",
            outline.id
        ))
        .await;

    // then: it is refused as no such kind
    refused_as(response, StatusCode::NOT_FOUND, refused::NO_SUCH_KIND);
}

#[tokio::test]
async fn an_outline_discarded_with_its_project_refuses_what_comes_after() {
    // given: an outline discarded with its project
    let (server, outlines) = a_server_and_its_outlines();
    let outline = an_open_outline(&server).await;
    outlines
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");

    // when: a section is added
    let response = server
        .post(&format!("/outlines/{}/sections", outline.id))
        .json(&AddSectionRequest {
            under: None,
            after: None,
            title: "Chapter 1".to_owned(),
        })
        .await;

    // then: it is refused as discarded
    refused_as(response, StatusCode::CONFLICT, refused::DISCARDED);
}

#[tokio::test]
async fn an_idea_of_another_project_is_not_found_by_an_outline() {
    // given: an outline with a chapter
    let (server, outlines) = a_server_and_its_outlines();
    let outline = an_open_outline(&server).await;
    let chapter = a_section(&server, &outline.id, "Chapter 1", None, None).await;
    // and: an idea of another project
    outlines
        .came_to_be(
            None,
            &outline_core::Attachment::idea("idea_strange"),
            "project_2",
        )
        .await
        .expect("remembering should succeed");

    // when: it is attached to the chapter
    let response = server
        .post(&format!("/outlines/{}/attachments", outline.id))
        .json(&AttachRequest {
            attachment: AttachmentDTO::Idea {
                id: "idea_strange".to_owned(),
            },
            section: chapter.section.clone(),
            after: None,
        })
        .await;

    // then: it is not found, as if there were no such idea, telling nothing of it
    refused_as(response, StatusCode::NOT_FOUND, "not-found");
}
