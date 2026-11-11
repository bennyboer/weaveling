use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::Agent;
use ideas_contract::{CAPTURED, DISCARDED};
use messaging::{Listener, Message, RoutingKey};
use outline_core::{Attachment, OutlineId, OutlinesError, SectionId, SectionTitle};
use scenes_contract::{CREATED, DELETED, SceneCreatedDTO, SceneDeletedDTO};
use serde_json::json;

use crate::wiring;
use crate::wiring::Wired;

const A_PROJECT: &str = "project_1";
const ANOTHER: &str = "project_2";

fn a_workbench() -> Wired {
    wiring::wired(Arc::new(FixedClock::new(clock::at(1_000))))
}

fn from_the_ideas(routing: &str, idea: &str, body: serde_json::Value) -> Message {
    eventpublishing::published(
        routing,
        "idea",
        idea,
        2,
        body,
        &Agent::System,
        clock::at(1_000),
    )
}

fn captured(idea: &str, project: &str) -> Message {
    from_the_ideas(
        CAPTURED,
        idea,
        json!({ "version": 0, "name": "CAPTURED", "project": project, "title": "An idea" }),
    )
}

fn discarded(idea: &str) -> Message {
    from_the_ideas(
        DISCARDED,
        idea,
        json!({ "version": 1, "name": "DISCARDED" }),
    )
}

fn from_the_scenes(routing: &str, payload: serde_json::Value) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key is fine"),
        payload,
        clock::at(1_000),
    )
}

fn created(scene: &str, project: &str) -> Message {
    from_the_scenes(
        CREATED,
        serde_json::to_value(SceneCreatedDTO {
            scene: scene.to_owned(),
            project: project.to_owned(),
        })
        .expect("a creation is plain data"),
    )
}

fn deleted(scene: &str) -> Message {
    from_the_scenes(
        DELETED,
        serde_json::to_value(SceneDeletedDTO {
            scene: scene.to_owned(),
        })
        .expect("a deletion is plain data"),
    )
}

async fn heard(listener: &Arc<dyn Listener>, message: Message) {
    listener
        .handle(&message)
        .await
        .expect("the listener should take this message");
}

async fn a_book(wired: &Wired) -> (OutlineId, SectionId) {
    let outline = wired
        .outlines
        .open(A_PROJECT, &eventsourcing::an_author())
        .await
        .expect("opening should succeed")
        .id;
    let chapter = wired
        .outlines
        .add(
            &outline.to_string(),
            None,
            None,
            SectionTitle::new("Chapter 1").expect("the title should be usable"),
            None,
            &eventsourcing::an_author(),
        )
        .await
        .expect("adding a section should succeed")
        .section;

    (outline, chapter)
}

async fn attached(
    wired: &Wired,
    (outline, chapter): (OutlineId, SectionId),
    attachment: Attachment,
) -> Result<(), OutlinesError> {
    wired
        .outlines
        .attach(
            &outline.to_string(),
            attachment,
            chapter,
            None,
            None,
            &eventsourcing::an_author(),
        )
        .await
        .map(|_| ())
}

#[tokio::test]
async fn an_idea_or_a_scene_of_another_project_is_not_there_for_an_outline() {
    // given: a book of one project
    let wired = a_workbench();
    let book = a_book(&wired).await;
    // and: an idea and a scene heard of in another, their ids known from elsewhere
    heard(
        &wired.remembering_projects,
        captured("idea_strange", ANOTHER),
    )
    .await;
    heard(
        &wired.remembering_projects,
        created("scene_strange", ANOTHER),
    )
    .await;

    // when: each is attached to the book
    let idea = attached(&wired, book, Attachment::idea("idea_strange")).await;
    let scene = attached(&wired, book, Attachment::scene("scene_strange")).await;

    // then: neither is there
    assert!(
        matches!(idea, Err(OutlinesError::AttachmentNotThere)),
        "an outline must not note an idea of another project, which is what let another project \
         learn where it appears: {idea:?}"
    );
    assert!(
        matches!(scene, Err(OutlinesError::AttachmentNotThere)),
        "nor hold a scene of another project: {scene:?}"
    );
}

#[tokio::test]
async fn an_idea_or_a_scene_of_the_book_s_project_or_not_heard_of_yet_is_attached() {
    // given: a book of one project
    let wired = a_workbench();
    let book = a_book(&wired).await;
    // and: an idea and a scene heard of in that project
    heard(&wired.remembering_projects, captured("idea_own", A_PROJECT)).await;
    heard(&wired.remembering_projects, created("scene_own", A_PROJECT)).await;

    // when: they and an idea and a scene not heard of yet are attached
    let attaching = [
        Attachment::idea("idea_own"),
        Attachment::scene("scene_own"),
        Attachment::idea("idea_fresh"),
        Attachment::scene("scene_fresh"),
    ];
    let mut outcomes = Vec::new();
    for attachment in attaching {
        outcomes.push(attached(&wired, book, attachment).await);
    }

    // then: every one is
    assert!(
        outcomes.iter().all(Result::is_ok),
        "one created a moment ago may be attached before its creation reaches the outline: \
         {outcomes:?}"
    );
}

#[tokio::test]
async fn a_discarded_idea_s_and_a_deleted_scene_s_project_are_forgotten() {
    // given: a book of one project
    let wired = a_workbench();
    let book = a_book(&wired).await;
    // and: an idea and a scene of another project, heard of and then gone
    heard(&wired.remembering_projects, captured("idea_gone", ANOTHER)).await;
    heard(&wired.remembering_projects, created("scene_gone", ANOTHER)).await;
    heard(&wired.on_idea_discarded, discarded("idea_gone")).await;
    heard(&wired.on_scene_deleted, deleted("scene_gone")).await;

    // when: each is attached to the book
    let idea = attached(&wired, book, Attachment::idea("idea_gone")).await;
    let scene = attached(&wired, book, Attachment::scene("scene_gone")).await;

    // then: their projects are no longer known to refuse them by
    assert!(
        !matches!(idea, Err(OutlinesError::AttachmentNotThere)),
        "nothing of a discarded idea, nor of a deleted project's ideas, stays with the outline"
    );
    assert!(
        !matches!(scene, Err(OutlinesError::AttachmentNotThere)),
        "nor of a deleted scene"
    );
}
