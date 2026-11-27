use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, EventMetadata, Version,
};
use projects_contract::context::PROJECT;
use time::{Duration, OffsetDateTime};

use std::sync::LazyLock;

use crate::about::{About, ReadingCopyPlace, Subject};
use crate::block_path::BlockPath;
use crate::comment::{Comment, CommentBlock, CommentRun};
use crate::id::FeedbackId;
use crate::link::{ProjectLink, ReadingCopyLink, SceneLink};
use crate::piece::*;

const DRAFTED_AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

fn ada() -> Agent {
    Agent::Account(AccountId::from("account_ada"))
}

fn bea() -> Agent {
    Agent::Account(AccountId::from("account_bea"))
}

fn saying(text: &str) -> Comment {
    Comment::new(vec![CommentBlock::Paragraph(vec![CommentRun::Text {
        text: text.to_owned(),
        emphasis: false,
        strong: false,
        link: None,
    }])])
    .expect("a plain comment is fine")
}

fn in_the_copy(place: ReadingCopyPlace) -> About {
    About::ReadingCopy {
        reading_copy: ReadingCopyLink::from("copy_loom"),
        place,
    }
}

fn a_passage(quote: &str) -> About {
    in_the_copy(ReadingCopyPlace::Passage {
        scene: SceneLink::from("scene_harbour"),
        block_path: BlockPath::new(vec![0]),
        from: 4,
        to: 9,
        quote: quote.to_owned(),
    })
}

/// Stamped as ada's, at a time so many minutes after the draft.
fn stamped(version: u64, minutes: i64) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("feedback_1"),
        kind: KIND,
        version: Version::of(version),
        agent: ada(),
        occurred_at: DRAFTED_AT + Duration::minutes(minutes),
        is_snapshot: false,
        context: AggregateContext::default(),
    }
}

fn a_drafting(about: About) -> FeedbackCommand {
    FeedbackCommand::Draft {
        project: ProjectLink::from("project_loom"),
        about,
        comment: saying("Which boats?"),
    }
}

/// Ada's draft on a passage.
fn a_draft() -> Feedback {
    let events =
        Feedback::begin(a_drafting(a_passage("boats")), &ada()).expect("drafting should succeed");
    let (first, rest) = events.split_first().expect("at least one event");
    let mut drafted =
        Feedback::from_first(first, &stamped(1, 0)).expect("the first event makes it");
    for event in rest {
        drafted.apply(event, &stamped(2, 0));
    }

    drafted
}

/// What a piece becomes once the agent's command is decided and its events applied, an hour after the draft.
fn decided(
    feedback: &Feedback,
    command: FeedbackCommand,
    by: &Agent,
) -> Result<Feedback, FeedbackRefused> {
    let events = feedback.decide(command, by)?;
    let mut changed = feedback.clone();
    for event in &events {
        changed.apply(event, &stamped(2, 60));
    }

    Ok(changed)
}

/// Ada's draft, submitted.
fn given() -> Feedback {
    decided(&a_draft(), FeedbackCommand::Submit, &ada()).expect("submitting should succeed")
}

#[test]
fn a_draft_is_its_writer_s_about_a_place_in_a_project() {
    // when: ada drafts a comment on a passage
    let drafted = a_draft();

    // then: it is a draft, ada's, about the passage, saying what ada wrote, when ada wrote it
    assert!(drafted.is_draft());
    assert_eq!(drafted.written_by(), &ada());
    assert_eq!(drafted.written_at(), DRAFTED_AT);
    assert_eq!(drafted.about(), Some(&a_passage("boats")));
    assert_eq!(drafted.comment(), Some(&saying("Which boats?")));
    // and: not done, not edited, and it tells its project
    assert!(!drafted.is_done());
    assert!(!drafted.is_edited());
    assert_eq!(drafted.context().get(PROJECT), Some("project_loom"));
}

#[test]
fn a_passage_holding_nothing_is_not_drafted_on() {
    // when: a passage quoting nothing is drafted on
    let refused = Feedback::begin(a_drafting(a_passage("  ")), &ada());

    // then: it is refused
    assert_eq!(refused, Err(FeedbackRefused::PassageEmpty));
}

#[test]
fn nothing_happens_to_feedback_before_it_is_drafted() {
    // expect: anything but a draft is refused before there is one
    for command in [
        FeedbackCommand::Reword(saying("Hm.")),
        FeedbackCommand::Submit,
        FeedbackCommand::MarkDone,
        FeedbackCommand::MarkNotDone,
        FeedbackCommand::Delete(Deleter::Writer),
        FeedbackCommand::Delete(Deleter::Owner),
    ] {
        assert_eq!(
            Feedback::begin(command.clone(), &ada()),
            Err(FeedbackRefused::NotDraftedYet),
            "{command:?}"
        );
    }
}

#[test]
fn a_draft_is_drafted_once() {
    // when: a draft is drafted again
    let refused = a_draft().decide(a_drafting(a_passage("boats")), &ada());

    // then: it is refused
    assert_eq!(refused, Err(FeedbackRefused::AlreadyDrafted));
}

#[test]
fn a_draft_reworded_is_not_edited_but_feedback_given_and_reworded_is() {
    // when: ada rewords the draft
    let reworded = decided(
        &a_draft(),
        FeedbackCommand::Reword(saying("Which ships?")),
        &ada(),
    )
    .expect("rewording should succeed");
    // then: it says the new words, and is not edited, as nobody saw the old ones
    assert_eq!(reworded.comment(), Some(&saying("Which ships?")));
    assert!(!reworded.is_edited());

    // when: ada rewords what ada gave
    let edited = decided(
        &given(),
        FeedbackCommand::Reword(saying("Which ships?")),
        &ada(),
    )
    .expect("rewording should succeed");
    // then: it says the new words, and is edited
    assert_eq!(edited.comment(), Some(&saying("Which ships?")));
    assert!(edited.is_edited());
}

#[test]
fn the_same_words_again_change_nothing() {
    // when: ada rewords what ada gave with the same words
    let events = given()
        .decide(FeedbackCommand::Reword(saying("Which boats?")), &ada())
        .expect("rewording should succeed");

    // then: nothing happens, and it is not edited
    assert!(events.is_empty());
}

#[test]
fn submitting_gives_the_feedback_when_it_is_submitted_and_twice_changes_nothing() {
    // when: ada submits the draft
    let submitted = given();

    // then: it is no draft anymore, submitted an hour after it was written
    assert!(!submitted.is_draft());
    assert_eq!(
        submitted.submitted_at(),
        Some(DRAFTED_AT + Duration::hours(1))
    );

    // when: ada submits it again
    let again = submitted
        .decide(FeedbackCommand::Submit, &ada())
        .expect("submitting again should succeed");
    // then: nothing happens
    assert!(again.is_empty());
}

#[test]
fn only_its_writer_rewords_submits_or_deletes_a_piece() {
    // expect: bea may not reword, submit or delete ada's draft or what ada gave
    for feedback in [a_draft(), given()] {
        for command in [
            FeedbackCommand::Reword(saying("Mine now.")),
            FeedbackCommand::Submit,
            FeedbackCommand::Delete(Deleter::Writer),
        ] {
            assert_eq!(
                feedback.decide(command.clone(), &bea()),
                Err(FeedbackRefused::NotYours),
                "{command:?} on a draft: {}",
                feedback.is_draft()
            );
        }
    }
}

#[test]
fn feedback_given_is_marked_done_and_not_done_again_by_anyone_the_service_lets_in() {
    // when: bea marks ada's feedback done
    let done =
        decided(&given(), FeedbackCommand::MarkDone, &bea()).expect("marking done should succeed");
    // then: it is done
    assert!(done.is_done());
    // and: marking it done again changes nothing
    assert!(
        done.decide(FeedbackCommand::MarkDone, &bea())
            .expect("marking done again should succeed")
            .is_empty()
    );

    // when: it is marked not done
    let not_done = decided(&done, FeedbackCommand::MarkNotDone, &bea())
        .expect("marking not done should succeed");
    // then: it is pending again
    assert!(!not_done.is_done());
    // and: marking it not done again changes nothing
    assert!(
        not_done
            .decide(FeedbackCommand::MarkNotDone, &bea())
            .expect("marking not done again should succeed")
            .is_empty()
    );
}

#[test]
fn a_draft_is_not_marked_done_nor_not_done() {
    // expect: a draft is not worked through, not even by its writer
    for command in [FeedbackCommand::MarkDone, FeedbackCommand::MarkNotDone] {
        assert_eq!(
            a_draft().decide(command.clone(), &ada()),
            Err(FeedbackRefused::NotSubmitted),
            "{command:?}"
        );
    }
}

/// The piece replied to, one and the same each time it is named.
static THE_ROOT: LazyLock<FeedbackId> =
    LazyLock::new(|| FeedbackId::generate(time::OffsetDateTime::UNIX_EPOCH));

fn the_root() -> FeedbackId {
    *THE_ROOT
}

/// Bea's reply to the root, given on the copy.
fn a_reply() -> Feedback {
    let events = Feedback::begin(
        FeedbackCommand::Reply {
            project: ProjectLink::from("project_loom"),
            to: the_root(),
            on: Subject::ReadingCopy(ReadingCopyLink::from("copy_loom")),
            comment: saying("The fishing boats."),
        },
        &bea(),
    )
    .expect("replying should succeed");
    let (first, rest) = events.split_first().expect("at least one event");
    let by_bea = |version| EventMetadata {
        agent: bea(),
        ..stamped(version, 0)
    };
    let mut replied = Feedback::from_first(first, &by_bea(1)).expect("the first event makes it");
    for event in rest {
        replied.apply(event, &by_bea(2));
    }

    replied
}

#[test]
fn a_reply_is_sent_at_once_on_its_root_in_the_root_s_subject() {
    // when: bea replies to the root
    let replied = a_reply();

    // then: it is no draft, sent when it was written
    assert!(!replied.is_draft());
    assert_eq!(replied.submitted_at(), Some(DRAFTED_AT));
    // and: it replies to the root, on the root's copy, and is in no place of its own
    let about = replied.about().expect("it is about something");
    assert!(replied.is_reply());
    assert_eq!(about.replying_to(), Some(&the_root()));
    assert_eq!(
        about.subject(),
        Subject::ReadingCopy(ReadingCopyLink::from("copy_loom"))
    );
    assert_eq!(about.in_scene(), None);
    assert_eq!(about.on_section(), None);
    // and: a comment given is no reply
    assert!(!given().is_reply());
}

#[test]
fn a_reply_is_never_marked_done_nor_not_done() {
    // expect: marking a reply is refused, as being done stays with its root
    for command in [FeedbackCommand::MarkDone, FeedbackCommand::MarkNotDone] {
        assert_eq!(
            a_reply().decide(command.clone(), &ada()),
            Err(FeedbackRefused::ReplyNotMarked),
            "{command:?}"
        );
    }
}

#[test]
fn a_reply_is_never_drafted_and_changed_by_its_writer_alone() {
    // expect: a draft about a piece of feedback is refused, as replies are sent at once
    assert_eq!(
        Feedback::begin(
            a_drafting(About::Feedback {
                feedback: the_root(),
                on: Subject::ReadingCopy(ReadingCopyLink::from("copy_loom")),
            }),
            &bea(),
        ),
        Err(FeedbackRefused::RepliesSentAtOnce)
    );
    // and: ada may not reword bea's reply
    assert_eq!(
        a_reply().decide(FeedbackCommand::Reword(saying("Not mine.")), &ada()),
        Err(FeedbackRefused::NotYours)
    );
    // and: bea rewords it, marked edited, as it was sent
    let reworded = decided(
        &a_reply(),
        FeedbackCommand::Reword(saying("The fishing boats, at dawn.")),
        &bea(),
    )
    .expect("rewording should succeed");
    assert!(reworded.is_edited());
}

#[test]
fn a_deleted_piece_accepts_nothing() {
    // given: ada's feedback, deleted by ada
    let deleted = decided(&given(), FeedbackCommand::Delete(Deleter::Writer), &ada())
        .expect("deleting should succeed");
    assert!(deleted.is_deleted());

    // expect: nothing else happens to it
    for command in [
        FeedbackCommand::Reword(saying("Again.")),
        FeedbackCommand::Submit,
        FeedbackCommand::MarkDone,
        FeedbackCommand::MarkNotDone,
        FeedbackCommand::Delete(Deleter::Writer),
        FeedbackCommand::Delete(Deleter::Owner),
        a_drafting(a_passage("boats")),
    ] {
        assert_eq!(
            deleted.decide(command.clone(), &ada()),
            Err(FeedbackRefused::Deleted),
            "{command:?}"
        );
    }
}

#[test]
fn a_snapshot_holds_the_whole_piece() {
    // given: ada's feedback, reworded after it was given and marked done
    let reworded = decided(
        &given(),
        FeedbackCommand::Reword(saying("Which ships?")),
        &ada(),
    )
    .expect("rewording should succeed");
    let done =
        decided(&reworded, FeedbackCommand::MarkDone, &bea()).expect("marking done should succeed");

    // when: it is snapshotted and grown again from the snapshot
    let grown =
        Feedback::from_first(&done.snapshot(), &stamped(9, 120)).expect("a snapshot makes it");

    // then: it is the same piece
    assert_eq!(grown, done);
}

#[test]
fn an_owner_deletes_anyone_s_feedback_given_but_no_draft() {
    // when: bea, found by the service to own the project, deletes what ada gave
    let deleted = decided(&given(), FeedbackCommand::Delete(Deleter::Owner), &bea())
        .expect("deleting as an owner should succeed");
    // then: it is deleted
    assert!(deleted.is_deleted());

    // when: bea deletes ada's draft as an owner
    let refused = a_draft().decide(FeedbackCommand::Delete(Deleter::Owner), &bea());
    // then: it is refused, as a draft is not there for anyone but its writer
    assert_eq!(refused, Err(FeedbackRefused::NotSubmitted));
}

#[test]
fn a_deleted_piece_holds_its_words_no_more_nor_does_its_snapshot() {
    // when: ada deletes what ada gave
    let deleted = decided(&given(), FeedbackCommand::Delete(Deleter::Writer), &ada())
        .expect("deleting should succeed");

    // then: it holds neither what it was about nor what it said
    assert_eq!(deleted.about(), None);
    assert_eq!(deleted.comment(), None);
    // and: its snapshot holds none of it either, only whose and in which project it was
    match deleted.snapshot() {
        FeedbackEvent::Snapshotted {
            project,
            about,
            comment,
            written_by,
            deleted,
            ..
        } => {
            assert_eq!(project, ProjectLink::from("project_loom"));
            assert_eq!((about, comment), (None, None));
            assert_eq!(written_by, ada());
            assert!(deleted);
        }
        other => panic!("a snapshot was expected, not {other:?}"),
    }
}

#[test]
fn the_system_deletes_drafts_and_feedback_given_alike() {
    // expect: the system deletes ada's draft and what ada gave, as their copy goes
    for feedback in [a_draft(), given()] {
        let deleted = decided(
            &feedback,
            FeedbackCommand::Delete(Deleter::System),
            &Agent::System,
        )
        .expect("deleting as the system should succeed");
        assert!(deleted.is_deleted(), "a draft: {}", feedback.is_draft());
    }
}
