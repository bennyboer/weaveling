use axum_test::TestServer;
use feedback_contract::{AboutDTO, FeedbackDTO, ReadingCopyPlaceDTO, RewriteFeedbackRequest};
use live::testing::Subscriber;
use serde_json::json;

use crate::support::{READING_COPY, a_passage, as_, drafted, feedback_followed, saying, socket_as};

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

    // then: bea is told it changed, and nothing of what it says
    let frame = bea.frame().await;
    assert_eq!(frame["of"], given.id);
    assert_eq!(frame["event"], json!({ "kind": "changed" }));

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
        assert_eq!(bea.frame().await["event"], json!({ "kind": "changed" }));
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
