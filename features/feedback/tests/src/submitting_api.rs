use axum::http::StatusCode;
use axum_test::TestServer;
use feedback_contract::{FeedbackDTO, ReadingCopyPlaceDTO, RewriteFeedbackRequest, refused};

use crate::support::{
    a_passage, as_, drafted, feedback, in_the_copy, on_the_copy, refusal, saying,
};

async fn submitted(server: &TestServer) -> Vec<FeedbackDTO> {
    let response = server
        .post("/feedback/submitted?kind=reading_copy&id=copy_loom")
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>()
}

#[tokio::test]
async fn drafts_are_submitted_together_and_then_seen_by_everyone() {
    // given: two drafts of ada's, and one of hedy's
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");
    let hedy = as_(&feedback, "account_hedy");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Whole),
        saying("Lovely."),
    )
    .await;
    drafted(&hedy, a_passage(), saying("Still thinking.")).await;

    // when: ada submits hers
    let sent = submitted(&ada).await;

    // then: both of hers are submitted, and no longer drafts
    assert_eq!(sent.len(), 2);
    for piece in &sent {
        assert!(!piece.draft);
        assert!(piece.by_you);
        assert!(piece.submitted_at.is_some());
        assert!(!piece.edited && !piece.done);
    }
    // and: hedy sees them, not hers, beside her own draft
    let seen = on_the_copy(&hedy).await;
    assert_eq!(seen.len(), 3);
    assert_eq!(
        seen.iter().filter(|piece| !piece.by_you).count(),
        2,
        "ada's two, submitted"
    );
    assert!(seen.iter().any(|piece| piece.by_you && piece.draft));
    // and: submitting again sends nothing more
    assert_eq!(submitted(&ada).await, vec![]);
}

#[tokio::test]
async fn submitted_feedback_is_marked_edited_once_changed_and_deleted_by_its_writer_alone() {
    // given: a comment of ada's, submitted
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");
    let hedy = as_(&feedback, "account_hedy");
    let given = drafted(&ada, a_passage(), saying("First thought.")).await;
    submitted(&ada).await;

    // when: she changes it
    let changed = ada
        .put(&format!("/feedback/{}/comment", given.id))
        .json(&RewriteFeedbackRequest {
            comment: saying("Second thought."),
        })
        .await;

    // then: it says the new words and is marked edited
    changed.assert_status_ok();
    let changed = changed.json::<FeedbackDTO>();
    assert_eq!(changed.comment, saying("Second thought."));
    assert!(changed.edited);
    // and: hedy may neither change nor delete it
    let rewriting = hedy
        .put(&format!("/feedback/{}/comment", given.id))
        .json(&RewriteFeedbackRequest {
            comment: saying("Not mine."),
        })
        .await;
    rewriting.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(refusal(&rewriting), refused::NOT_YOURS);
    hedy.delete(&format!("/feedback/{}", given.id))
        .await
        .assert_status(StatusCode::FORBIDDEN);

    // when: ada deletes it
    ada.delete(&format!("/feedback/{}", given.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: nobody finds it
    assert_eq!(on_the_copy(&hedy).await, vec![]);
}
