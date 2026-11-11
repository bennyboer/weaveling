use std::sync::Arc;

use boards_core::{BoardId, BoardsError, IdeaLink, Size, Spot};
use clock::FixedClock;
use eventsourcing::{Agent, Standing};
use ideas_contract::{CAPTURED, DISCARDED};
use messaging::Message;
use serde_json::json;

use crate::wiring;
use crate::wiring::Wired;

fn a_workbench() -> Wired {
    wiring::wired(Arc::new(FixedClock::new(clock::at(1_000))))
}

fn from_the_ideas(routing: &str, idea: &str, body: serde_json::Value) -> Message {
    eventpublishing::published(
        routing,
        "idea",
        idea,
        4,
        body,
        &Agent::Local,
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

async fn the_board_of_project_1(wired: &Wired) -> BoardId {
    wired
        .boards
        .open("project_1", &eventsourcing::an_author())
        .await
        .expect("opening should succeed")
        .id
}

async fn pinned(
    wired: &Wired,
    board: &BoardId,
    idea: &str,
) -> Result<Standing<boards_core::Board>, BoardsError> {
    wired
        .boards
        .pin(
            &board.to_string(),
            IdeaLink::from(idea),
            Spot::ORIGIN,
            Size::CARD,
            None,
            &eventsourcing::an_author(),
        )
        .await
}

async fn heard(listener: &Arc<dyn messaging::Listener>, message: Message) {
    listener
        .handle(&message)
        .await
        .expect("the listener should take this message");
}

#[tokio::test]
async fn an_idea_of_another_project_is_not_there_for_a_board() {
    // given: a board of one project
    let wired = a_workbench();
    let board = the_board_of_project_1(&wired).await;
    // and: an idea heard captured in another, its id known from elsewhere
    heard(
        &wired.remembering_projects_of_ideas,
        captured("idea_strange", "project_2"),
    )
    .await;

    // when: it is pinned to the board
    let refused = pinned(&wired, &board, "idea_strange").await;

    // then: it is not there
    assert!(
        matches!(refused, Err(BoardsError::IdeaNotThere)),
        "a board must not hold an idea of another project, which is what let another project \
         learn where it appears"
    );
}

#[tokio::test]
async fn an_idea_of_the_board_s_project_or_not_heard_of_yet_is_pinned() {
    // given: a board of one project
    let wired = a_workbench();
    let board = the_board_of_project_1(&wired).await;
    // and: an idea heard captured in that project
    heard(
        &wired.remembering_projects_of_ideas,
        captured("idea_own", "project_1"),
    )
    .await;

    // when: it and an idea not heard of yet are pinned
    let own = pinned(&wired, &board, "idea_own").await;
    let fresh = pinned(&wired, &board, "idea_fresh").await;

    // then: both are
    assert!(own.is_ok(), "{own:?}");
    assert!(
        fresh.is_ok(),
        "a board pins an idea it captured a moment ago, before the capture reaches it: {fresh:?}"
    );
}

#[tokio::test]
async fn a_discarded_idea_s_project_is_forgotten() {
    // given: a board of one project
    let wired = a_workbench();
    let board = the_board_of_project_1(&wired).await;
    // and: an idea of another project, heard captured and then discarded
    heard(
        &wired.remembering_projects_of_ideas,
        captured("idea_gone", "project_2"),
    )
    .await;
    heard(&wired.on_idea_discarded, discarded("idea_gone")).await;

    // when: it is pinned to the board
    let pinning = pinned(&wired, &board, "idea_gone").await;

    // then: its project is no longer known to refuse it by
    assert!(
        !matches!(pinning, Err(BoardsError::IdeaNotThere)),
        "nothing of a discarded idea, nor of a deleted project's ideas, stays with the boards"
    );
}
