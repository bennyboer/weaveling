use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use feedback_contract::{
    AboutDTO, DraftFeedbackRequest, FeedbackDTO, ReadingCopyPlaceDTO, RewriteFeedbackRequest,
    refused,
};
use feedback_core::ReadingCopyLink;

use crate::support::{
    CLOSING_COPY, NOW, READING_COPY, TickingClock, a_passage, as_, drafted, feedback, feedback_on,
    refusal, saying,
};

async fn given(server: &TestServer, comment: &str) -> FeedbackDTO {
    drafted(server, a_passage(), saying(comment)).await;
    let response = server
        .post(&format!(
            "/feedback/submitted?subjects=reading_copy:{READING_COPY}"
        ))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>().remove(0)
}

async fn replying(server: &TestServer, to: &str, comment: &str) -> axum_test::TestResponse {
    server
        .post("/feedback")
        .json(&DraftFeedbackRequest {
            about: AboutDTO::Feedback {
                feedback: to.to_owned(),
            },
            comment: saying(comment),
        })
        .await
}

async fn replied(server: &TestServer, to: &str, comment: &str) -> FeedbackDTO {
    let response = replying(server, to, comment).await;
    response.assert_status(StatusCode::CREATED);

    response.json::<FeedbackDTO>()
}

async fn thread_of(server: &TestServer, root: &str) -> Vec<FeedbackDTO> {
    let response = server
        .get(&format!("/feedback/on?subjects=feedback:{root}"))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>()
}

#[tokio::test]
async fn a_reply_is_sent_at_once_and_listed_in_its_thread_oldest_first() {
    // given: a comment ada gave
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let otto = as_(&feedback, "account_otto");
    let root = given(&ada, "Which boats?").await;

    // when: bea and then otto reply to it
    let first = replied(&bea, &root.id, "The fishing boats.").await;
    replied(&otto, &root.id, "I'll make it clearer.").await;

    // then: bea's reply was sent at once, on the comment
    assert!(!first.draft);
    assert!(first.submitted_at.is_some());
    assert_eq!(
        first.about,
        AboutDTO::Feedback {
            feedback: root.id.clone()
        }
    );
    // and: the thread lists both, oldest first, by whom
    let thread = thread_of(&ada, &root.id).await;
    let said: Vec<_> = thread
        .iter()
        .map(|reply| (reply.written_by.clone(), reply.by_you))
        .collect();
    assert_eq!(said, vec![(Some("Bea".to_owned()), false), (None, false)]);
    // and: the comment counts its replies where it is listed
    let listed = ada.get("/feedback/on?project=project_loom").await;
    assert_eq!(listed.json::<Vec<FeedbackDTO>>()[0].replies, 2);
}

#[tokio::test]
async fn a_reply_to_a_reply_joins_the_thread_of_the_comment_it_began_from() {
    // given: a comment ada gave, and bea's reply to it
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let root = given(&ada, "Which boats?").await;
    let reply = replied(&bea, &root.id, "The fishing boats.").await;

    // when: ada replies to bea's reply
    let answer = replied(&ada, &reply.id, "Thanks!").await;

    // then: hers is on the comment, last in its thread
    assert_eq!(
        answer.about,
        AboutDTO::Feedback {
            feedback: root.id.clone()
        }
    );
    let thread = thread_of(&bea, &root.id).await;
    assert_eq!(
        thread.iter().map(|one| &one.id).collect::<Vec<_>>(),
        vec![&reply.id, &answer.id]
    );
}

#[tokio::test]
async fn a_reply_is_changed_and_deleted_by_whoever_wrote_it_alone() {
    // given: bea's reply to ada's comment
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let hedy = as_(&feedback, "account_hedy");
    let root = given(&ada, "Which boats?").await;
    let reply = replied(&bea, &root.id, "The fishing boats.").await;

    // when: hedy tries to change and delete it
    let changed_by_hedy = hedy
        .put(&format!("/feedback/{}/comment", reply.id))
        .json(&RewriteFeedbackRequest {
            comment: saying("Not mine."),
        })
        .await;
    let deleted_by_hedy = hedy.delete(&format!("/feedback/{}", reply.id)).await;

    // then: both are refused as not hers
    changed_by_hedy.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(refusal(&changed_by_hedy), refused::NOT_YOURS);
    deleted_by_hedy.assert_status(StatusCode::FORBIDDEN);

    // when: bea changes it
    let changed = bea
        .put(&format!("/feedback/{}/comment", reply.id))
        .json(&RewriteFeedbackRequest {
            comment: saying("The fishing boats, at dawn."),
        })
        .await;

    // then: it says so, marked edited
    changed.assert_status_ok();
    assert!(changed.json::<FeedbackDTO>().edited);

    // when: bea deletes it
    bea.delete(&format!("/feedback/{}", reply.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: the thread is empty
    assert!(thread_of(&ada, &root.id).await.is_empty());
}

#[tokio::test]
async fn nobody_replies_to_or_reads_the_thread_of_what_they_cannot_see() {
    // given: a comment ada gave, and a draft of hers
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let grace = as_(&feedback, "account_grace");
    let root = given(&ada, "Which boats?").await;
    let draft = drafted(&ada, a_passage(), saying("Still thinking.")).await;

    // when: bea replies to ada's draft
    let to_a_draft = replying(&bea, &draft.id, "Me too.").await;
    // then: it is not there for her
    to_a_draft.assert_status(StatusCode::NOT_FOUND);

    // when: grace, no member of the project, replies to the comment and asks for its thread
    let by_grace = replying(&grace, &root.id, "Hello.").await;
    let read_by_grace = grace
        .get(&format!("/feedback/on?subjects=feedback:{}", root.id))
        .await;
    // then: it is not there for her either
    by_grace.assert_status(StatusCode::NOT_FOUND);
    read_by_grace.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_blank_reply_is_refused() {
    // given: a comment ada gave
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let root = given(&ada, "Which boats?").await;

    // when: bea replies with nothing to say
    let response = replying(&bea, &root.id, "   ").await;

    // then: it is refused as blank
    response.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&response), refused::COMMENT_BLANK);
}

/// The pieces named, as whoever asks finds them: those gone are left out.
async fn still_there(server: &TestServer, pieces: &[&str]) -> Vec<String> {
    let response = server
        .get(&format!("/feedback/pieces?ids={}", pieces.join(",")))
        .await;
    response.assert_status_ok();

    response
        .json::<Vec<FeedbackDTO>>()
        .into_iter()
        .map(|piece| piece.id)
        .collect()
}

#[tokio::test]
async fn an_owner_deletes_someone_else_s_reply_and_an_author_may_not() {
    // given: bea's reply to otto's comment
    let feedback = feedback().await;
    let otto = as_(&feedback, "account_otto");
    let bea = as_(&feedback, "account_bea");
    let ada = as_(&feedback, "account_ada");
    let hedy = as_(&feedback, "account_hedy");
    let root = given(&otto, "Which boats?").await;
    let reply = replied(&bea, &root.id, "The fishing boats.").await;

    // when: hedy, an author, deletes it
    let by_hedy = hedy.delete(&format!("/feedback/{}", reply.id)).await;
    // then: it is not hers to delete
    by_hedy.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(refusal(&by_hedy), refused::NOT_YOURS);

    // when: ada, an owner, deletes it
    ada.delete(&format!("/feedback/{}", reply.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // then: the thread is empty
    assert!(thread_of(&otto, &root.id).await.is_empty());
}

#[tokio::test]
async fn a_reply_is_never_marked_done_as_being_done_stays_with_its_comment() {
    // given: bea's reply to ada's comment
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let root = given(&ada, "Which boats?").await;
    let reply = replied(&bea, &root.id, "The fishing boats.").await;

    // when: ada, an owner, marks it done
    let marked = ada.put(&format!("/feedback/{}/done", reply.id)).await;

    // then: it is refused
    marked.assert_status(StatusCode::CONFLICT);
    assert_eq!(refusal(&marked), refused::REPLY_NOT_MARKED);
}

#[tokio::test]
async fn a_comment_deleted_takes_its_replies_with_it() {
    // given: ada's comment, with two replies
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let root = given(&ada, "Which boats?").await;
    let first = replied(&bea, &root.id, "The fishing boats.").await;
    let second = replied(&ada, &root.id, "Thanks!").await;

    // when: ada deletes her comment
    ada.delete(&format!("/feedback/{}", root.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: its replies are gone with it
    assert!(
        still_there(&bea, &[&root.id, &first.id, &second.id])
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn a_swept_reading_copy_takes_the_replies_given_in_it_with_it() {
    // given: a comment in the copy, with a reply
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let root = given(&ada, "Which boats?").await;
    let reply = replied(&bea, &root.id, "The fishing boats.").await;

    // when: the copy is deleted and swept
    feedback
        .sweep_reading_copy(&ReadingCopyLink::from(READING_COPY))
        .await
        .expect("sweeping should succeed");

    // then: the comment and its reply are gone
    assert!(still_there(&bea, &[&root.id, &reply.id]).await.is_empty());
}

#[tokio::test]
async fn nobody_replies_once_the_copy_the_comment_was_given_in_closed() {
    // given: ada's comment given in a copy that closes a day from now
    let clock = Arc::new(TickingClock::at(clock::at(NOW)));
    let feedback = feedback_on(clock.clone()).await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    drafted(
        &ada,
        AboutDTO::ReadingCopy {
            reading_copy: CLOSING_COPY.to_owned(),
            place: ReadingCopyPlaceDTO::Whole,
        },
        saying("Lovely."),
    )
    .await;
    let root = ada
        .post(&format!(
            "/feedback/submitted?subjects=reading_copy:{CLOSING_COPY}"
        ))
        .await
        .json::<Vec<FeedbackDTO>>()
        .remove(0);

    // when: two days pass, and bea replies to it
    clock.set(clock::at(NOW + 2 * 24 * 60 * 60));
    let replying_late = replying(&bea, &root.id, "Agreed.").await;

    // then: the copy is closed for it
    replying_late.assert_status(StatusCode::CONFLICT);
    assert_eq!(refusal(&replying_late), refused::READING_COPY_CLOSED);
}
