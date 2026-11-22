use axum::http::StatusCode;
use feedback_contract::{
    CommentBlockDTO, CommentRunDTO, DraftFeedbackRequest, FeedbackDTO, ReadingCopyPlaceDTO,
    RewriteFeedbackRequest, refused,
};

use crate::support::{
    a_passage, as_, drafted, feedback, in_the_copy, on_the_copy, refusal, saying,
};

#[tokio::test]
async fn a_draft_is_written_about_a_passage_a_scene_a_section_or_the_whole_copy() {
    // given: the feedback
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");

    // when: ada drafts on each kind of place
    let mut made = Vec::new();
    for about in [
        a_passage(),
        in_the_copy(ReadingCopyPlaceDTO::Scene {
            scene: "scene_harbour".to_owned(),
        }),
        in_the_copy(ReadingCopyPlaceDTO::Section {
            section: "section_morning".to_owned(),
        }),
        in_the_copy(ReadingCopyPlaceDTO::Whole),
    ] {
        made.push(drafted(&ada, about, saying("This stopped me.")).await);
    }

    // then: each is a draft of hers, about what she chose
    for draft in &made {
        assert!(draft.draft);
        assert!(draft.by_you);
        assert_eq!(draft.comment, saying("This stopped me."));
    }
    assert_eq!(made[0].about, a_passage());
    assert_eq!(made[3].about, in_the_copy(ReadingCopyPlaceDTO::Whole));
    // and: she finds them on the copy, in the order she wrote them
    assert_eq!(on_the_copy(&ada).await, made);
}

#[tokio::test]
async fn a_draft_is_seen_by_whoever_wrote_it_alone() {
    // given: a draft of ada's
    let feedback = feedback();
    let draft = drafted(&as_(&feedback, "account_ada"), a_passage(), saying("Mine.")).await;
    let hedy = as_(&feedback, "account_hedy");

    // expect: hedy finds nothing on the copy
    assert_eq!(on_the_copy(&hedy).await, vec![]);
    // and: to her it is not there to rewrite or delete
    hedy.put(&format!("/feedback/{}/comment", draft.id))
        .json(&RewriteFeedbackRequest {
            comment: saying("Not mine."),
        })
        .await
        .assert_status(StatusCode::NOT_FOUND);
    hedy.delete(&format!("/feedback/{}", draft.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_draft_is_rewritten_and_deleted_by_whoever_wrote_it() {
    // given: a draft of ada's
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");
    let draft = drafted(&ada, a_passage(), saying("First thought.")).await;
    // and: what she says now, formatted
    let now = vec![
        CommentBlockDTO::Paragraph {
            runs: vec![
                CommentRunDTO::Text {
                    text: "Second".to_owned(),
                    emphasis: true,
                    strong: true,
                    link: None,
                },
                CommentRunDTO::Break,
                CommentRunDTO::Text {
                    text: " thought, see ".to_owned(),
                    emphasis: false,
                    strong: false,
                    link: None,
                },
                CommentRunDTO::Text {
                    text: "here".to_owned(),
                    emphasis: false,
                    strong: false,
                    link: Some("https://example.org".to_owned()),
                },
            ],
        },
        CommentBlockDTO::List {
            ordered: true,
            items: vec![
                vec![CommentRunDTO::Text {
                    text: "one".to_owned(),
                    emphasis: false,
                    strong: false,
                    link: None,
                }],
                vec![],
            ],
        },
    ];

    // when: she rewrites it
    let rewritten = ada
        .put(&format!("/feedback/{}/comment", draft.id))
        .json(&RewriteFeedbackRequest {
            comment: now.clone(),
        })
        .await;

    // then: it says what she says now, as she formatted it
    rewritten.assert_status_ok();
    assert_eq!(rewritten.json::<FeedbackDTO>().comment, now);
    assert_eq!(on_the_copy(&ada).await[0].comment, now);

    // when: she deletes it
    ada.delete(&format!("/feedback/{}", draft.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is gone
    assert_eq!(on_the_copy(&ada).await, vec![]);
}

#[tokio::test]
async fn a_blank_draft_an_unsafe_link_or_a_passage_holding_nothing_is_refused() {
    // given: the feedback
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");

    // expect: a draft saying nothing is refused
    let blank = ada
        .post("/feedback")
        .json(&DraftFeedbackRequest {
            about: in_the_copy(ReadingCopyPlaceDTO::Whole),
            comment: saying("  "),
        })
        .await;
    blank.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&blank), refused::COMMENT_BLANK);
    // and: a link to a script is refused
    let scripted = ada
        .post("/feedback")
        .json(&DraftFeedbackRequest {
            about: in_the_copy(ReadingCopyPlaceDTO::Whole),
            comment: vec![CommentBlockDTO::Paragraph {
                runs: vec![CommentRunDTO::Text {
                    text: "click".to_owned(),
                    emphasis: false,
                    strong: false,
                    link: Some("javascript:alert(1)".to_owned()),
                }],
            }],
        })
        .await;
    scripted.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&scripted), refused::COMMENT_LINK_UNSAFE);
    // and: a passage of no characters is refused
    let empty = ada
        .post("/feedback")
        .json(&DraftFeedbackRequest {
            about: in_the_copy(ReadingCopyPlaceDTO::Passage {
                scene: "scene_harbour".to_owned(),
                block_path: vec![0],
                from: 4,
                to: 4,
                quote: String::new(),
            }),
            comment: saying("Hm."),
        })
        .await;
    empty.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refusal(&empty), refused::PASSAGE_EMPTY);
    // and: nothing was drafted
    assert_eq!(on_the_copy(&ada).await, vec![]);
}

#[tokio::test]
async fn drafts_are_asked_for_by_where_they_lie_and_are_neither_pending_nor_done() {
    // given: a draft of ada's on the copy
    let feedback = feedback();
    let ada = as_(&feedback, "account_ada");
    let draft = drafted(&ada, a_passage(), saying("Mine.")).await;

    // expect: she finds it on its scene as on its copy
    let on_scene = ada.get("/feedback/drafts?places=scene:scene_harbour").await;
    on_scene.assert_status_ok();
    assert_eq!(on_scene.json::<Vec<FeedbackDTO>>(), vec![draft]);
    // and: drafts asked for by status are refused, as they have none
    let by_status = ada
        .get("/feedback/drafts?places=scene:scene_harbour&status=pending")
        .await;
    by_status.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(refusal(&by_status), refused::STATUS_UNKNOWN);
}
