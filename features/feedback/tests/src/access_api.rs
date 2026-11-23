use axum::http::StatusCode;
use axum_test::TestServer;
use feedback_contract::{
    AboutDTO, DraftFeedbackRequest, FeedbackDTO, ReadingCopyPlaceDTO, refused,
};

use eventsourcing::{AggregateId, EventStore, Version};
use feedback_core::{FeedbackEvent, KIND};

use crate::support::{
    READING_COPY, a_passage, as_, drafted, feedback, feedback_with_events, refusal, saying,
};

/// One's drafts in the loom copy, submitted together.
async fn given(server: &TestServer) -> Vec<FeedbackDTO> {
    let response = server
        .post(&format!(
            "/feedback/submitted?subjects=reading_copy:{READING_COPY}"
        ))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>()
}

#[tokio::test]
async fn to_someone_who_is_no_member_a_project_s_feedback_is_not_there() {
    // given: ada's feedback given in the loom project, and zed, who is no member of it
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let zed = as_(&feedback, "account_zed");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    let boats = given(&ada).await.remove(0);

    // expect: the project's feedback, its copy's, its givers, its counts and what is pending are not there for zed
    for asked in [
        "/feedback/on?project=project_loom".to_owned(),
        format!("/feedback/on?subjects=reading_copy:{READING_COPY}"),
        format!("/feedback/drafts?subjects=reading_copy:{READING_COPY}"),
        "/feedback/givers?project=project_loom".to_owned(),
        format!("/feedback/counts?subjects=reading_copy:{READING_COPY}"),
        "/feedback/pending?project=project_loom".to_owned(),
    ] {
        zed.get(&asked).await.assert_status(StatusCode::NOT_FOUND);
    }
    // and: zed may neither draft in the copy nor mark ada's feedback done
    zed.post("/feedback")
        .json(&DraftFeedbackRequest {
            about: a_passage(),
            comment: saying("Me too."),
        })
        .await
        .assert_status(StatusCode::NOT_FOUND);
    zed.put(&format!("/feedback/{}/done", boats.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_reader_gives_feedback_and_the_authors_mark_it_done() {
    // given: bea, who reads the loom project, giving feedback in it
    let feedback = feedback().await;
    let bea = as_(&feedback, "account_bea");
    let otto = as_(&feedback, "account_otto");
    drafted(&bea, a_passage(), saying("Which boats?")).await;
    let boats = given(&bea).await.remove(0);

    // when: bea marks it done
    let by_a_reader = bea.put(&format!("/feedback/{}/done", boats.id)).await;

    // then: it is refused, as only the authors and owners work feedback through
    by_a_reader.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(refusal(&by_a_reader), refused::MARKED_BY_AUTHORS);

    // when: otto, who writes the project, marks it done
    let by_an_author = otto.put(&format!("/feedback/{}/done", boats.id)).await;

    // then: it is done
    by_an_author.assert_status_ok();
    assert!(by_an_author.json::<FeedbackDTO>().done);
}

#[tokio::test]
async fn an_owner_deletes_anyone_s_feedback_given_but_an_author_does_not() {
    // given: bea's feedback given, and bea's draft
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let otto = as_(&feedback, "account_otto");
    drafted(&bea, a_passage(), saying("Which boats?")).await;
    let boats = given(&bea).await.remove(0);
    let draft = drafted(
        &bea,
        AboutDTO::ReadingCopy {
            reading_copy: READING_COPY.to_owned(),
            place: ReadingCopyPlaceDTO::Whole,
        },
        saying("Still thinking."),
    )
    .await;

    // when: otto, who writes the project but does not own it, deletes the feedback given
    let by_an_author = otto.delete(&format!("/feedback/{}", boats.id)).await;

    // then: it is refused as not theirs
    by_an_author.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(refusal(&by_an_author), refused::NOT_YOURS);

    // when: ada, who owns the project, deletes it
    ada.delete(&format!("/feedback/{}", boats.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is gone for everyone
    let left = bea.get("/feedback/on?project=project_loom").await;
    assert!(left.json::<Vec<FeedbackDTO>>().is_empty());
    // and: bea's draft is not there for ada to delete
    ada.delete(&format!("/feedback/{}", draft.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn feedback_asked_for_in_no_project_and_on_no_copy_is_refused() {
    // given: ada
    let ada = as_(&feedback().await, "account_ada");

    // when: feedback is asked for on a scene alone, which may lie in any project
    let response = ada.get("/feedback/on?places=scene:scene_harbour").await;

    // then: it is refused as no place named that tells whose it is
    response.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&response), refused::PLACE_UNNAMED);
}

#[tokio::test]
async fn a_piece_deleted_leaves_no_word_of_it_behind() {
    // given: ada's feedback given on a passage
    let (feedback, events) = feedback_with_events().await;
    let ada = as_(&feedback, "account_ada");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    let boats = given(&ada).await.remove(0);

    // when: ada deletes it
    ada.delete(&format!("/feedback/{}", boats.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: its events collapsed into one snapshot, which holds neither the passage nor the comment
    let kept = events
        .read_from(&AggregateId::from(boats.id.as_str()), KIND, Version::ZERO)
        .await
        .expect("reading the stream should succeed");
    assert_eq!(
        kept.len(),
        1,
        "only the snapshot of the deleted piece stays"
    );
    match &kept[0].event {
        FeedbackEvent::Snapshotted {
            about,
            comment,
            deleted,
            ..
        } => {
            assert_eq!((about, comment), (&None, &None));
            assert!(deleted);
        }
        other => panic!("a snapshot was expected, not {other:?}"),
    }
}
