use axum::http::StatusCode;
use axum_test::TestServer;
use feedback_contract::{
    AboutDTO, COUNTED_AT_ONCE, CommentBlockDTO, CommentRunDTO, FeedbackCountDTO, FeedbackDTO,
    PendingDTO, ReadingCopyPlaceDTO, RewriteFeedbackRequest, refused,
};

use crate::support::{
    a_passage, as_, drafted, feedback, in_the_copy, on_the_copy, refusal, saying,
};

async fn submitted(server: &TestServer) -> Vec<FeedbackDTO> {
    let response = server
        .post("/feedback/submitted?reading_copies=copy_loom")
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
            "/feedback/counts?reading_copies={}",
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
            "/feedback/counts?reading_copies={}",
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

#[tokio::test]
async fn a_scene_s_feedback_is_found_in_every_copy_on_it_and_on_passages_in_it_but_no_draft() {
    // given: in two copies, a passage and the scene commented on and submitted
    let feedback = feedback();
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
    ada.post("/feedback/submitted?reading_copies=copy_weft")
        .await
        .assert_status_ok();
    // and: a draft on the scene still
    drafted(&ada, a_passage(), saying("Still thinking.")).await;

    // when: the scene's feedback is asked for
    let response = ada.get("/feedback/on?scenes=scene_harbour").await;

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
async fn a_section_s_feedback_is_found_on_it_alone_and_what_is_not_done_is_counted_by_scene_and_section()
 {
    // given: a passage and its scene, a section, and the whole copy commented on and submitted, and a draft on the section
    let feedback = feedback();
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
    let found = ada.get("/feedback/on?sections=section_morning").await;

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
async fn feedback_is_asked_for_in_a_project_on_scenes_or_on_sections_one_of_them() {
    // given: the feedback
    let ada = as_(&feedback(), "account_ada");

    // expect: neither, or both, are refused as no place named
    for asked in [
        "/feedback/on",
        "/feedback/on?scenes=scene_harbour&sections=section_morning",
        "/feedback/on?project=project_loom&scenes=scene_harbour",
    ] {
        let response = ada.get(asked).await;
        response.assert_status(StatusCode::BAD_REQUEST);
        assert_eq!(refusal(&response), refused::PLACE_UNNAMED, "{asked}");
    }
}

#[tokio::test]
async fn a_project_s_feedback_given_is_found_in_every_copy_of_it_but_no_draft() {
    // given: a comment given in one copy and another, a draft, and a comment in a copy of another project
    let feedback = feedback();
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
    ada.post("/feedback/submitted?reading_copies=copy_weft")
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
    ada.post("/feedback/submitted?reading_copies=copy_shuttle")
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
    let feedback = feedback();
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
    let on_both = "/feedback/on?scenes=scene_harbour,scene_market";
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
        .map(|at| format!("scene_{at}"))
        .collect();
    let refused_many = ada
        .get(&format!("/feedback/on?scenes={}", many.join(",")))
        .await;
    refused_many.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&refused_many), refused::TOO_MANY_COUNTED);
}

#[tokio::test]
async fn a_project_s_feedback_is_paged_newest_or_oldest_first_and_counted_whole() {
    // given: three comments submitted one after another, the last on the whole copy
    let feedback = feedback();
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
