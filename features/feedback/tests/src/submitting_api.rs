use axum::http::StatusCode;
use axum_test::TestServer;
use feedback_contract::{
    COUNTED_AT_ONCE, FeedbackCountDTO, FeedbackDTO, ReadingCopyPlaceDTO, RewriteFeedbackRequest,
    refused,
};

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

async fn counted(server: &TestServer, ids: &[&str]) -> Vec<FeedbackCountDTO> {
    let response = server
        .get(&format!(
            "/feedback/counts?kind=reading_copy&ids={}",
            ids.join(",")
        ))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackCountDTO>>()
}

#[tokio::test]
async fn each_copy_counts_what_is_not_done_and_your_drafts_on_it_in_the_order_asked() {
    // given: two comments of ada's submitted on the copy, and a draft of hers and of hedy's after
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");
    let hedy = as_(&feedback, "account_hedy");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    drafted(&ada, a_passage(), saying("Lovely.")).await;
    submitted(&ada).await;
    drafted(&ada, a_passage(), saying("Still thinking.")).await;
    drafted(&hedy, a_passage(), saying("Mine.")).await;

    // when: ada counts the copy and another
    let seen = counted(&ada, &["copy_weft", "copy_loom"]).await;

    // then: each is counted in the order asked, her own drafts alone among the drafts
    assert_eq!(
        seen,
        vec![
            FeedbackCountDTO {
                id: "copy_weft".to_owned(),
                not_done: 0,
                your_drafts: 0,
            },
            FeedbackCountDTO {
                id: "copy_loom".to_owned(),
                not_done: 2,
                your_drafts: 1,
            },
        ]
    );
}

#[tokio::test]
async fn no_more_copies_are_counted_at_once_than_the_contract_says() {
    // given: as many copies as are counted at once, and one more
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");
    let ids: Vec<String> = (0..=COUNTED_AT_ONCE)
        .map(|at| format!("copy_{at}"))
        .collect();
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();

    // expect: as many as the contract says are counted
    assert_eq!(
        counted(&ada, &ids[..COUNTED_AT_ONCE]).await.len(),
        COUNTED_AT_ONCE
    );
    // and: one more is refused
    let refused_one_more = ada
        .get(&format!(
            "/feedback/counts?kind=reading_copy&ids={}",
            ids.join(",")
        ))
        .await;
    refused_one_more.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&refused_one_more), refused::TOO_MANY_COUNTED);
}

#[tokio::test]
async fn feedback_given_is_marked_done_and_not_done_again_but_a_draft_is_not_there_for_it() {
    // given: a comment of ada's submitted, and a draft of hers
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");
    let hedy = as_(&feedback, "account_hedy");
    let given = drafted(&ada, a_passage(), saying("Which boats?")).await;
    submitted(&ada).await;
    let draft = drafted(&ada, a_passage(), saying("Still thinking.")).await;

    // when: hedy marks the comment done
    let marked = hedy.put(&format!("/feedback/{}/done", given.id)).await;

    // then: it is done, for everyone, and counted no more as not done
    marked.assert_status_ok();
    assert!(marked.json::<FeedbackDTO>().done);
    assert!(
        on_the_copy(&ada)
            .await
            .iter()
            .any(|piece| piece.id == given.id && piece.done)
    );
    assert_eq!(counted(&ada, &["copy_loom"]).await[0].not_done, 0);

    // when: it is marked not done again
    let unmarked = hedy.delete(&format!("/feedback/{}/done", given.id)).await;

    // then: it is not done
    unmarked.assert_status_ok();
    assert!(!unmarked.json::<FeedbackDTO>().done);
    // and: a draft is not there to be marked
    ada.put(&format!("/feedback/{}/done", draft.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
