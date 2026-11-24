use axum::http::StatusCode;
use axum_test::TestServer;
use feedback_contract::{
    AboutDTO, COUNTED_AT_ONCE, FeedbackDTO, ReadingCopyPlaceDTO, RewriteFeedbackRequest, refused,
};
use live::testing::Subscriber;
use serde_json::json;

use crate::support::{
    READING_COPY, a_passage, as_, drafted, feedback_followed, refusal, saying, socket_as,
};

const THE_LOOM: &str = "project_loom";

async fn submitted_in(server: &TestServer, copy: &str) -> Vec<FeedbackDTO> {
    let response = server
        .post(&format!("/feedback/submitted?subjects=reading_copy:{copy}"))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>()
}

/// Follows the loom's feedback as an account, kept with the server it follows on.
async fn following_the_loom(server: &TestServer) -> Subscriber {
    let mut follower = Subscriber::on(server).await;
    follower
        .subscribe("feedback", "feedback", &[("project", THE_LOOM)])
        .await;
    follower.following("feedback").await;

    follower
}

#[tokio::test]
async fn whoever_may_read_the_project_is_told_of_feedback_given_there_as_it_changes_and_goes() {
    // given: bea, a reader, following the loom's feedback, and a draft of ada's
    let (feedback, broadcaster) = feedback_followed().await;
    let socket = socket_as(&feedback, &broadcaster, "account_bea");
    let mut bea = following_the_loom(&socket).await;
    let ada = as_(&feedback, "account_ada");
    drafted(&ada, a_passage(), saying("Which boats?")).await;

    // when: ada submits it
    let given = submitted_in(&ada, READING_COPY).await.remove(0);

    // then: bea is told it changed in the copy, and nothing of what it says
    let frame = bea.frame().await;
    assert_eq!(frame["of"], given.id);
    assert_eq!(
        frame["event"],
        json!({ "kind": "changed", "subject": "reading_copy:copy_loom" })
    );

    // when: ada rewords it, and marks it done
    ada.put(&format!("/feedback/{}/comment", given.id))
        .json(&RewriteFeedbackRequest {
            comment: saying("Which ships?"),
        })
        .await
        .assert_status_ok();
    ada.put(&format!("/feedback/{}/done", given.id))
        .await
        .assert_status_ok();

    // then: bea is told it changed, each time
    for _ in 0..2 {
        assert_eq!(bea.frame().await["event"]["kind"], "changed");
    }

    // when: ada deletes it
    ada.delete(&format!("/feedback/{}", given.id))
        .await
        .assert_status_no_content();

    // then: bea is told it is gone
    let frame = bea.frame().await;
    assert_eq!(frame["of"], given.id);
    assert_eq!(frame["event"], json!({ "kind": "gone" }));
}

#[tokio::test]
async fn a_draft_is_never_told_of_while_it_is_written_changed_and_deleted() {
    // given: bea following the loom's feedback
    let (feedback, broadcaster) = feedback_followed().await;
    let socket = socket_as(&feedback, &broadcaster, "account_bea");
    let mut bea = following_the_loom(&socket).await;
    let ada = as_(&feedback, "account_ada");

    // when: ada drafts, rewords and deletes a draft
    let draft = drafted(&ada, a_passage(), saying("Which boats?")).await;
    ada.put(&format!("/feedback/{}/comment", draft.id))
        .json(&RewriteFeedbackRequest {
            comment: saying("Which ships?"),
        })
        .await
        .assert_status_ok();
    ada.delete(&format!("/feedback/{}", draft.id))
        .await
        .assert_status_no_content();

    // then: bea is told nothing
    bea.nothing().await;
}

#[tokio::test]
async fn feedback_given_in_another_project_is_not_told() {
    // given: bea following the loom's feedback
    let (feedback, broadcaster) = feedback_followed().await;
    let socket = socket_as(&feedback, &broadcaster, "account_bea");
    let mut bea = following_the_loom(&socket).await;

    // when: ada gives feedback in the shuttle
    let ada = as_(&feedback, "account_ada");
    drafted(
        &ada,
        AboutDTO::ReadingCopy {
            reading_copy: "copy_shuttle".to_owned(),
            place: ReadingCopyPlaceDTO::Whole,
        },
        saying("Lovely."),
    )
    .await;
    submitted_in(&ada, "copy_shuttle").await;

    // then: bea is told nothing
    bea.nothing().await;
}

#[tokio::test]
async fn whoever_may_not_read_the_project_cannot_follow_its_feedback_as_if_it_were_not_there() {
    // given: grace, who is no member of the loom
    let (feedback, broadcaster) = feedback_followed().await;
    let socket = socket_as(&feedback, &broadcaster, "account_grace");
    let mut grace = Subscriber::on(&socket).await;

    // when: grace follows the loom's feedback
    grace
        .subscribe("feedback", "feedback", &[("project", THE_LOOM)])
        .await;

    // then: she is told it is not there
    assert_eq!(grace.ended().await, "not-there");
}

async fn pieces(server: &TestServer, ids: &[&str]) -> Vec<FeedbackDTO> {
    let response = server
        .get(&format!("/feedback/pieces?ids={}", ids.join(",")))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>()
}

#[tokio::test]
async fn the_pieces_heard_of_are_asked_for_by_their_ids_in_the_order_asked() {
    // given: two pieces ada gave in the loom
    let (feedback, _) = feedback_followed().await;
    let ada = as_(&feedback, "account_ada");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    drafted(&ada, a_passage(), saying("Lovely.")).await;
    let given = submitted_in(&ada, READING_COPY).await;
    let (boats, lovely) = (&given[0].id, &given[1].id);

    // when: bea asks for them, the later first
    let bea = as_(&feedback, "account_bea");
    let found = pieces(&bea, &[lovely, boats]).await;

    // then: she finds both, in the order asked, as ada's
    let ids: Vec<&str> = found.iter().map(|piece| piece.id.as_str()).collect();
    assert_eq!(ids, vec![lovely.as_str(), boats.as_str()]);
    assert!(found.iter().all(|piece| !piece.by_you));
    assert_eq!(found[0].written_by.as_deref(), Some("Ada"));
}

#[tokio::test]
async fn a_piece_not_there_for_the_asker_is_left_out_of_the_pieces_asked_for() {
    // given: a piece ada gave in the loom, her draft there, and a piece she gave in the shuttle, which bea may not read
    let (feedback, _) = feedback_followed().await;
    let ada = as_(&feedback, "account_ada");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    let given = submitted_in(&ada, READING_COPY).await.remove(0);
    let draft = drafted(&ada, a_passage(), saying("Still thinking.")).await;
    drafted(
        &ada,
        AboutDTO::ReadingCopy {
            reading_copy: "copy_shuttle".to_owned(),
            place: ReadingCopyPlaceDTO::Whole,
        },
        saying("Lovely."),
    )
    .await;
    let elsewhere = submitted_in(&ada, "copy_shuttle").await.remove(0);

    // when: bea asks for all of them, and for a piece that never was
    let bea = as_(&feedback, "account_bea");
    let found = pieces(&bea, &[&draft.id, "not-a-piece", &elsewhere.id, &given.id]).await;

    // then: she finds only the piece given where she may read
    let ids: Vec<&str> = found.iter().map(|piece| piece.id.as_str()).collect();
    assert_eq!(ids, vec![given.id.as_str()]);
}

#[tokio::test]
async fn more_pieces_than_are_counted_at_once_are_refused() {
    // given: more ids than are counted at once
    let (feedback, _) = feedback_followed().await;
    let bea = as_(&feedback, "account_bea");
    let ids: Vec<String> = (0..=COUNTED_AT_ONCE)
        .map(|n| format!("piece_{n}"))
        .collect();

    // when: they are asked for together
    let response = bea
        .get(&format!("/feedback/pieces?ids={}", ids.join(",")))
        .await;

    // then: it is refused
    response.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&response), refused::TOO_MANY_COUNTED);
}
