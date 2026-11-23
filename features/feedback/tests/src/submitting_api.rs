use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use feedback_contract::{
    AboutDTO, COUNTED_AT_ONCE, CommentBlockDTO, CommentRunDTO, FeedbackCountDTO, FeedbackDTO,
    ListedGiverDTO, PendingDTO, ReadingCopyPlaceDTO, RewriteFeedbackRequest, refused,
};

use std::sync::Arc;

use feedback_contract::DraftFeedbackRequest;

use eventsourcing::AccountId;

use crate::support::{
    CLOSING_COPY, NOW, READING_COPY, TickingClock, a_passage, as_, drafted, feedback, feedback_on,
    in_the_copy, on_the_copy, refusal, saying,
};

async fn submitted(server: &TestServer) -> Vec<FeedbackDTO> {
    let response = server
        .post("/feedback/submitted?subjects=reading_copy:copy_loom")
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackDTO>>()
}

#[tokio::test]
async fn drafts_are_submitted_together_and_then_seen_by_everyone() {
    // given: two drafts of ada's, and one of hedy's
    let feedback = feedback().await;
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
    let feedback = feedback().await;
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
            "/feedback/counts?subjects={}",
            ids.iter()
                .map(|id| format!("reading_copy:{id}"))
                .collect::<Vec<_>>()
                .join(",")
        ))
        .await;
    response.assert_status_ok();

    response.json::<Vec<FeedbackCountDTO>>()
}

#[tokio::test]
async fn each_copy_counts_what_is_not_done_and_your_drafts_on_it_in_the_order_asked() {
    // given: two comments of ada's submitted on the copy, and a draft of hers and of hedy's after
    let feedback = feedback().await;
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
    let feedback = feedback().await;
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
            "/feedback/counts?subjects={}",
            ids.iter()
                .map(|id| format!("reading_copy:{id}"))
                .collect::<Vec<_>>()
                .join(",")
        ))
        .await;
    refused_one_more.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&refused_one_more), refused::TOO_MANY_COUNTED);
}

#[tokio::test]
async fn feedback_given_is_marked_done_and_not_done_again_but_a_draft_is_not_there_for_it() {
    // given: a comment of ada's submitted, and a draft of hers
    let feedback = feedback().await;
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

#[tokio::test]
async fn a_scene_s_feedback_is_found_in_every_copy_on_it_and_on_passages_in_it_but_no_draft() {
    // given: in two copies, a passage and the scene commented on and submitted
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let in_another = |place| AboutDTO::ReadingCopy {
        reading_copy: "copy_weft".to_owned(),
        place,
    };
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    drafted(
        &ada,
        in_another(ReadingCopyPlaceDTO::Scene {
            scene: "scene_harbour".to_owned(),
        }),
        saying("Slow."),
    )
    .await;
    // and: one on another scene and one on the whole copy
    drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Scene {
            scene: "scene_market".to_owned(),
        }),
        saying("Elsewhere."),
    )
    .await;
    drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Whole),
        saying("Lovely."),
    )
    .await;
    submitted(&ada).await;
    ada.post("/feedback/submitted?subjects=reading_copy:copy_weft")
        .await
        .assert_status_ok();
    // and: a draft on the scene still
    drafted(&ada, a_passage(), saying("Still thinking.")).await;

    // when: the scene's feedback is asked for
    let response = ada
        .get("/feedback/on?project=project_loom&places=scene:scene_harbour")
        .await;

    // then: the passage and the scene are found, from both copies, and nothing else
    response.assert_status_ok();
    let found: Vec<String> = response
        .json::<Vec<FeedbackDTO>>()
        .iter()
        .map(|piece| match &piece.comment[..] {
            [CommentBlockDTO::Paragraph { runs }] => match &runs[..] {
                [CommentRunDTO::Text { text, .. }] => text.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        })
        .collect();
    assert_eq!(found, ["Which boats?", "Slow."]);
}

#[tokio::test]
async fn a_section_asked_for_with_its_scenes_finds_the_feedback_on_any_of_them() {
    // given: a passage in a scene, the section itself, another scene, and the whole copy commented on and submitted
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let boats = drafted(&ada, a_passage(), saying("Which boats?")).await;
    let long = drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Section {
            section: "section_morning".to_owned(),
        }),
        saying("Long."),
    )
    .await;
    drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Scene {
            scene: "scene_market".to_owned(),
        }),
        saying("Elsewhere."),
    )
    .await;
    drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Whole),
        saying("Lovely."),
    )
    .await;
    submitted(&ada).await;

    // when: the section and the scene in it are asked for together
    let found = ada
        .get("/feedback/on?project=project_loom&places=section:section_morning,scene:scene_harbour")
        .await;

    // then: the passage in the scene and the section itself are found, and nothing else
    found.assert_status_ok();
    let found: Vec<String> = found
        .json::<Vec<FeedbackDTO>>()
        .into_iter()
        .map(|piece| piece.id)
        .collect();
    assert_eq!(found, [boats.id, long.id]);
}

#[tokio::test]
async fn a_section_s_feedback_is_found_on_it_alone_and_what_is_not_done_is_counted_by_scene_and_section()
 {
    // given: a passage and its scene, a section, and the whole copy commented on and submitted, and a draft on the section
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let the_section = || {
        in_the_copy(ReadingCopyPlaceDTO::Section {
            section: "section_morning".to_owned(),
        })
    };
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Scene {
            scene: "scene_harbour".to_owned(),
        }),
        saying("Slow."),
    )
    .await;
    let on_section = drafted(&ada, the_section(), saying("Long.")).await;
    drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Whole),
        saying("Lovely."),
    )
    .await;
    submitted(&ada).await;
    drafted(&ada, the_section(), saying("Still thinking.")).await;

    // when: the section's feedback is asked for
    let found = ada
        .get("/feedback/on?project=project_loom&places=section:section_morning")
        .await;

    // then: what was given on the section itself is found, and no draft
    found.assert_status_ok();
    let found = found.json::<Vec<FeedbackDTO>>();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, on_section.id);

    // when: what is not done in the project is counted
    let counted = ada.get("/feedback/pending?project=project_loom").await;

    // then: the scene holds two, the section one, the whole copy no place, and all four are counted together
    counted.assert_status_ok();
    assert_eq!(
        counted.json::<PendingDTO>(),
        PendingDTO {
            scenes: [("scene_harbour".to_owned(), 2)].into(),
            sections: [("section_morning".to_owned(), 1)].into(),
            total: 4,
        }
    );
}

#[tokio::test]
async fn feedback_asked_for_nowhere_in_particular_or_on_what_is_not_known_is_refused() {
    // given: the feedback
    let ada = as_(&feedback().await, "account_ada");

    // expect: nothing named, a subject of no known kind, or a place of no known kind are refused
    for (asked, refused_as) in [
        ("/feedback/on?status=pending", refused::PLACE_UNNAMED),
        (
            "/feedback/on?subjects=character:mira",
            refused::SUBJECT_UNKNOWN,
        ),
        ("/feedback/on?subjects=copy_loom", refused::SUBJECT_UNKNOWN),
        ("/feedback/on?places=chapter:one", refused::PLACE_UNKNOWN),
        (
            "/feedback/counts?subjects=board:one",
            refused::SUBJECT_UNKNOWN,
        ),
    ] {
        let response = ada.get(asked).await;
        response.assert_status(StatusCode::BAD_REQUEST);
        assert_eq!(refusal(&response), refused_as, "{asked}");
    }
}

#[tokio::test]
async fn a_project_copies_and_places_in_the_text_narrow_the_feedback_together() {
    // given: the same scene commented on in two copies of the project, and in a copy of another project
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let on_the_harbour = |copy: &str| AboutDTO::ReadingCopy {
        reading_copy: copy.to_owned(),
        place: ReadingCopyPlaceDTO::Scene {
            scene: "scene_harbour".to_owned(),
        },
    };
    let in_loom = drafted(&ada, on_the_harbour("copy_loom"), saying("Slow.")).await;
    let in_weft = drafted(&ada, on_the_harbour("copy_weft"), saying("Still slow.")).await;
    drafted(&ada, on_the_harbour("copy_shuttle"), saying("Elsewhere.")).await;
    for copy in ["copy_loom", "copy_weft", "copy_shuttle"] {
        ada.post(&format!("/feedback/submitted?subjects=reading_copy:{copy}"))
            .await
            .assert_status_ok();
    }
    let found = |response: TestResponse| -> Vec<String> {
        response.assert_status_ok();
        response
            .json::<Vec<FeedbackDTO>>()
            .into_iter()
            .map(|piece| piece.id)
            .collect()
    };

    // when: the scene is asked for in the project
    let in_the_project = ada
        .get("/feedback/on?project=project_loom&places=scene:scene_harbour")
        .await;
    // then: both copies of the project are found, not the other project's
    assert_eq!(found(in_the_project), [in_loom.id.clone(), in_weft.id]);

    // when: the scene is asked for in one copy
    let in_one_copy = ada
        .get("/feedback/on?subjects=reading_copy:copy_loom&places=scene:scene_harbour")
        .await;
    // then: only that copy's is found
    assert_eq!(found(in_one_copy), [in_loom.id]);
}

#[tokio::test]
async fn a_project_s_feedback_given_is_found_in_every_copy_of_it_but_no_draft() {
    // given: a comment given in one copy and another, a draft, and a comment in a copy of another project
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    drafted(
        &ada,
        AboutDTO::ReadingCopy {
            reading_copy: "copy_weft".to_owned(),
            place: ReadingCopyPlaceDTO::Whole,
        },
        saying("Lovely."),
    )
    .await;
    submitted(&ada).await;
    ada.post("/feedback/submitted?subjects=reading_copy:copy_weft")
        .await
        .assert_status_ok();
    drafted(&ada, a_passage(), saying("Still thinking.")).await;
    drafted(
        &ada,
        AboutDTO::ReadingCopy {
            reading_copy: "copy_shuttle".to_owned(),
            place: ReadingCopyPlaceDTO::Whole,
        },
        saying("Elsewhere."),
    )
    .await;
    ada.post("/feedback/submitted?subjects=reading_copy:copy_shuttle")
        .await
        .assert_status_ok();

    // when: the project's feedback is asked for
    let found = ada.get("/feedback/on?project=project_loom").await;

    // then: both given in it are found, and neither the draft nor the other project's comment
    found.assert_status_ok();
    let found = found.json::<Vec<FeedbackDTO>>();
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|piece| !piece.draft));

    // and: only the two are counted pending in it
    let pending = ada.get("/feedback/pending?project=project_loom").await;
    assert_eq!(pending.json::<PendingDTO>().total, 2);
}

#[tokio::test]
async fn the_feedback_pending_on_some_scenes_is_found_at_once_and_no_more_scenes_than_the_contract_says()
 {
    // given: passages in two scenes commented on, one marked done, the scene itself, and a draft, all in a copy
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let in_the_market = in_the_copy(ReadingCopyPlaceDTO::Passage {
        scene: "scene_market".to_owned(),
        block_path: vec![1],
        from: 0,
        to: 4,
        quote: "fish".to_owned(),
    });
    let boats = drafted(&ada, a_passage(), saying("Which boats?")).await;
    let done = drafted(&ada, in_the_market.clone(), saying("Done with.")).await;
    let slow = drafted(
        &ada,
        in_the_copy(ReadingCopyPlaceDTO::Scene {
            scene: "scene_harbour".to_owned(),
        }),
        saying("Slow."),
    )
    .await;
    submitted(&ada).await;
    ada.put(&format!("/feedback/{}/done", done.id))
        .await
        .assert_status_ok();
    drafted(&ada, in_the_market, saying("Still thinking.")).await;
    let on_both = "/feedback/on?project=project_loom&places=scene:scene_harbour,scene:scene_market";
    let ids = |found: Vec<FeedbackDTO>| found.into_iter().map(|piece| piece.id).collect::<Vec<_>>();

    // when: what is pending on both scenes is asked for
    let pending = ada.get(&format!("{on_both}&status=pending")).await;

    // then: the passage and the scene pending are found, and nothing done, nor the draft
    pending.assert_status_ok();
    assert_eq!(ids(pending.json()), [boats.id.clone(), slow.id.clone()]);
    // and: what is done is found when asked for
    assert_eq!(
        ids(ada.get(&format!("{on_both}&status=done")).await.json()),
        std::slice::from_ref(&done.id)
    );
    // and: all of it without a status
    assert_eq!(
        ids(ada.get(on_both).await.json()),
        [boats.id, done.id, slow.id]
    );
    // and: a status unheard of is refused
    let unheard = ada.get(&format!("{on_both}&status=urgent")).await;
    unheard.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&unheard), refused::STATUS_UNKNOWN);
    // and: no more scenes than the contract says are looked on at once
    let many: Vec<String> = (0..=COUNTED_AT_ONCE)
        .map(|at| format!("scene:scene_{at}"))
        .collect();
    let refused_many = ada
        .get(&format!(
            "/feedback/on?project=project_loom&places={}",
            many.join(",")
        ))
        .await;
    refused_many.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&refused_many), refused::TOO_MANY_COUNTED);
}

#[tokio::test]
async fn a_project_s_feedback_is_paged_newest_or_oldest_first_and_counted_whole() {
    // given: three comments submitted one after another, the last on the whole copy
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let mut ids = Vec::new();
    for (place, said) in [
        (a_passage(), "First."),
        (
            in_the_copy(ReadingCopyPlaceDTO::Scene {
                scene: "scene_harbour".to_owned(),
            }),
            "Second.",
        ),
        (in_the_copy(ReadingCopyPlaceDTO::Whole), "Third."),
    ] {
        ids.push(drafted(&ada, place, saying(said)).await.id);
        submitted(&ada).await;
    }
    let listed =
        |found: Vec<FeedbackDTO>| found.into_iter().map(|piece| piece.id).collect::<Vec<_>>();

    // when: two of them are asked for newest first
    let first = ada
        .get("/feedback/on?project=project_loom&order=newest&limit=2")
        .await;

    // then: the two newest come
    first.assert_status_ok();
    assert_eq!(listed(first.json()), [ids[2].clone(), ids[1].clone()]);
    // and: the page after them holds the oldest
    let next = ada
        .get(&format!(
            "/feedback/on?project=project_loom&order=newest&limit=2&after={}",
            ids[1]
        ))
        .await;
    assert_eq!(listed(next.json()), [ids[0].clone()]);
    // and: oldest first unless asked
    assert_eq!(
        listed(ada.get("/feedback/on?project=project_loom").await.json()),
        ids
    );
    // and: an order unheard of, or more than the contract says at once, is refused
    let unheard = ada
        .get("/feedback/on?project=project_loom&order=random")
        .await;
    unheard.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&unheard), refused::ORDER_UNKNOWN);
    let too_many = ada
        .get(&format!(
            "/feedback/on?project=project_loom&limit={}",
            COUNTED_AT_ONCE + 1
        ))
        .await;
    too_many.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&too_many), refused::TOO_MANY_COUNTED);
    // and: all three are counted pending, the one on the whole copy too
    let pending = ada.get("/feedback/pending?project=project_loom").await;
    assert_eq!(pending.json::<PendingDTO>().total, 3);
}

#[tokio::test]
async fn whoever_gave_feedback_in_a_project_is_listed_as_a_giver_and_their_feedback_found_alone() {
    // given: ada and then bea commenting in the project, ada again, and bea's draft not yet submitted
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let boats = drafted(&ada, a_passage(), saying("Which boats?")).await;
    submitted(&ada).await;
    let slow = drafted(&bea, a_passage(), saying("Slow.")).await;
    submitted(&bea).await;
    let lovely = drafted(&ada, a_passage(), saying("Lovely.")).await;
    submitted(&ada).await;
    drafted(&bea, a_passage(), saying("Still thinking.")).await;

    // when: bea asks who gave feedback in the project
    let givers = bea.get("/feedback/givers?project=project_loom").await;

    // then: ada and bea by the names they signed up with, in the order they first gave any, bea told which one is bea
    givers.assert_status_ok();
    assert_eq!(
        givers.json::<Vec<ListedGiverDTO>>(),
        [
            ListedGiverDTO {
                giver: "account:account_ada".to_owned(),
                by_you: false,
                name: Some("Ada".to_owned()),
            },
            ListedGiverDTO {
                giver: "account:account_bea".to_owned(),
                by_you: true,
                name: Some("Bea".to_owned()),
            },
        ]
    );

    // when: the project's feedback is asked for by ada alone, and by bea alone
    let by_ada = bea
        .get("/feedback/on?project=project_loom&given_by=account:account_ada")
        .await;
    let by_bea = bea
        .get("/feedback/on?project=project_loom&given_by=account:account_bea")
        .await;

    // then: each finds what that reader submitted, not bea's draft
    let ids = |response: axum_test::TestResponse| -> Vec<String> {
        response.assert_status_ok();
        response
            .json::<Vec<FeedbackDTO>>()
            .into_iter()
            .map(|piece| piece.id)
            .collect()
    };
    assert_eq!(ids(by_ada), [boats.id, lovely.id]);
    assert_eq!(ids(by_bea), [slow.id]);
    // and: what is pending is counted by the same narrowing
    let pending_by_ada = bea
        .get("/feedback/pending?project=project_loom&given_by=account:account_ada")
        .await;
    pending_by_ada.assert_status_ok();
    assert_eq!(pending_by_ada.json::<PendingDTO>().total, 2);

    // and: a giver of no known kind is refused
    let unknown = bea.get("/feedback/on?given_by=guest:someone").await;
    unknown.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&unknown), refused::GIVER_UNKNOWN);
}

#[tokio::test]
async fn feedback_is_drafted_and_submitted_only_while_its_copy_is_open() {
    // given: a draft in a copy that closes a day from now
    let clock = Arc::new(TickingClock::at(clock::at(NOW)));
    let feedback = feedback_on(clock.clone()).await;
    let ada = as_(&feedback, "account_ada");
    let in_the_closing_copy = || AboutDTO::ReadingCopy {
        reading_copy: CLOSING_COPY.to_owned(),
        place: ReadingCopyPlaceDTO::Whole,
    };
    drafted(&ada, in_the_closing_copy(), saying("Lovely.")).await;

    // when: two days pass, and the draft is submitted
    clock.set(clock::at(NOW + 2 * 24 * 60 * 60));
    let submitting = ada
        .post(&format!(
            "/feedback/submitted?subjects=reading_copy:{CLOSING_COPY}"
        ))
        .await;

    // then: it is refused, as the copy is closed
    submitting.assert_status(StatusCode::CONFLICT);
    assert_eq!(refusal(&submitting), refused::READING_COPY_CLOSED);

    // when: another draft is begun in it
    let drafting = ada
        .post("/feedback")
        .json(&DraftFeedbackRequest {
            about: in_the_closing_copy(),
            comment: saying("Too late?"),
        })
        .await;

    // then: it is refused too
    drafting.assert_status(StatusCode::CONFLICT);
    assert_eq!(refusal(&drafting), refused::READING_COPY_CLOSED);
    // and: a copy open without end still takes feedback
    drafted(&ada, a_passage(), saying("Which boats?")).await;
}

#[tokio::test]
async fn feedback_names_whoever_gave_it_until_their_account_is_deleted() {
    // given: ada's feedback given
    let feedback = feedback().await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    drafted(&ada, a_passage(), saying("Which boats?")).await;
    submitted(&ada).await;
    let named_by = |found: Vec<FeedbackDTO>| -> Vec<Option<String>> {
        found.into_iter().map(|piece| piece.written_by).collect()
    };

    // when: bea reads the copy's feedback
    let seen = bea
        .get(&format!(
            "/feedback/on?subjects=reading_copy:{READING_COPY}"
        ))
        .await;

    // then: it names ada by the name ada signed up with
    seen.assert_status_ok();
    assert_eq!(named_by(seen.json()), [Some("Ada".to_owned())]);

    // when: ada's account is deleted, and the feedback read again
    feedback
        .account_gone(None, &AccountId::from("account_ada"), clock::at(NOW))
        .await
        .expect("forgetting a name should succeed");
    let seen = bea
        .get(&format!(
            "/feedback/on?subjects=reading_copy:{READING_COPY}"
        ))
        .await;

    // then: the feedback stays, naming nobody
    seen.assert_status_ok();
    assert_eq!(named_by(seen.json()), [None]);
    // and: ada is listed as a giver still, under no name
    let givers = bea.get("/feedback/givers?project=project_loom").await;
    assert_eq!(givers.json::<Vec<ListedGiverDTO>>()[0].name, None);
}

#[tokio::test]
async fn a_draft_on_a_copy_the_feedback_has_not_heard_of_yet_is_refused_to_be_asked_again() {
    // given: a copy the readings have not told the feedback of yet
    let ada = as_(&feedback().await, "account_ada");

    // when: a draft is begun in it
    let drafting = ada
        .post("/feedback")
        .json(&DraftFeedbackRequest {
            about: AboutDTO::ReadingCopy {
                reading_copy: "copy_just_made".to_owned(),
                place: ReadingCopyPlaceDTO::Whole,
            },
            comment: saying("Lovely."),
        })
        .await;

    // then: it is refused as not known yet, to be asked again shortly
    drafting.assert_status(StatusCode::CONFLICT);
    assert_eq!(refusal(&drafting), refused::READING_COPY_UNKNOWN);
}

#[tokio::test]
async fn drafts_left_in_a_copy_once_it_closes_are_dropped_and_feedback_given_stays() {
    // given: in the copy closing a day from now ada's feedback given and drafts of ada and bea, and in an open copy a draft
    let clock = Arc::new(TickingClock::at(clock::at(NOW)));
    let feedback = feedback_on(clock.clone()).await;
    let ada = as_(&feedback, "account_ada");
    let bea = as_(&feedback, "account_bea");
    let in_the_closing_copy = || AboutDTO::ReadingCopy {
        reading_copy: CLOSING_COPY.to_owned(),
        place: ReadingCopyPlaceDTO::Whole,
    };
    drafted(&ada, in_the_closing_copy(), saying("Lovely.")).await;
    ada.post(&format!(
        "/feedback/submitted?subjects=reading_copy:{CLOSING_COPY}"
    ))
    .await
    .assert_status_ok();
    drafted(&ada, in_the_closing_copy(), saying("And then?")).await;
    drafted(&bea, in_the_closing_copy(), saying("Hm.")).await;
    let open = drafted(&ada, a_passage(), saying("Which boats?")).await;

    // when: two days pass, and the drafts of closed copies are dropped
    clock.set(clock::at(NOW + 2 * 24 * 60 * 60));
    let closed = feedback
        .drop_drafts_of_closed_copies(10)
        .await
        .expect("dropping should succeed");

    // then: the closing copy was closed out, its drafts gone, what was given there kept
    assert_eq!(closed, 1);
    for (who, left) in [(&ada, 0), (&bea, 0)] {
        assert_eq!(
            who.get(&format!(
                "/feedback/drafts?subjects=reading_copy:{CLOSING_COPY}"
            ))
            .await
            .json::<Vec<FeedbackDTO>>()
            .len(),
            left
        );
    }
    assert_eq!(
        ada.get(&format!(
            "/feedback/on?subjects=reading_copy:{CLOSING_COPY}"
        ))
        .await
        .json::<Vec<FeedbackDTO>>()
        .len(),
        1
    );
    // and: the draft in the open copy stays
    let left: Vec<String> = ada
        .get(&format!(
            "/feedback/drafts?subjects=reading_copy:{READING_COPY}"
        ))
        .await
        .json::<Vec<FeedbackDTO>>()
        .into_iter()
        .map(|piece| piece.id)
        .collect();
    assert_eq!(left, [open.id]);
    // and: asked again, nothing is left to do
    assert_eq!(
        feedback
            .drop_drafts_of_closed_copies(10)
            .await
            .expect("dropping again should succeed"),
        0
    );
}
