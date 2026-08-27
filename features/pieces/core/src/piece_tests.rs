use eventsourcing::{Agent, AgentId, Aggregate, AggregateId, Event, EventMetadata, Version};
use time::OffsetDateTime;

use crate::piece::*;
use crate::title::PieceTitle;

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_title(saying: &str) -> PieceTitle {
    PieceTitle::new(saying).expect("a plain title is fine")
}

fn stamped(version: u64) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("piece_1"),
        kind: KIND,
        version: Version::of(version),
        agent: an_author(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        is_snapshot: false,
    }
}

fn a_capture() -> PieceCommand {
    PieceCommand::Capture {
        project: ProjectLink::from("project_1"),
        title: a_title("The Loom"),
    }
}

fn a_captured_piece() -> Piece {
    let events = Piece::begin(a_capture(), &an_author()).expect("capturing should succeed");

    grown(&events)
}

fn grown(events: &[PieceEvent]) -> Piece {
    let (first, rest) = events.split_first().expect("at least one event");
    let mut piece = Piece::born(first, &stamped(1)).expect("the first event gives birth");

    for (counted, event) in rest.iter().enumerate() {
        piece.absorb(event, &stamped(counted as u64 + 2));
    }

    piece
}

#[test]
fn a_piece_is_captured_with_a_project_and_a_title() {
    let events = Piece::begin(a_capture(), &an_author()).expect("capturing should succeed");

    assert_eq!(
        events,
        vec![PieceEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        }]
    );
}

#[test]
fn a_piece_may_be_captured_with_no_title_at_all() {
    let events = Piece::begin(
        PieceCommand::Capture {
            project: ProjectLink::from("project_1"),
            title: PieceTitle::untitled(),
        },
        &an_author(),
    )
    .expect("an idea arrives before its name");

    assert_eq!(grown(&events).title(), &PieceTitle::untitled());
}

#[test]
fn nothing_but_a_capture_can_start_a_piece() {
    assert_eq!(
        Piece::begin(PieceCommand::Discard, &an_author()),
        Err(PieceError::NotCapturedYet)
    );
    assert_eq!(
        Piece::begin(PieceCommand::Retitle(a_title("Too soon")), &an_author()),
        Err(PieceError::NotCapturedYet)
    );
}

#[test]
fn a_captured_piece_holds_no_passage_yet() {
    let piece = a_captured_piece();

    assert_eq!(
        piece.passage(),
        None,
        "a passage is attached on first write"
    );
    assert!(!piece.is_discarded());
    assert_eq!(piece.project(), &ProjectLink::from("project_1"));
}

#[test]
fn retitling_records_the_title_it_became() {
    let decided = a_captured_piece()
        .decide(
            PieceCommand::Retitle(a_title("The Silent Loom")),
            &an_author(),
        )
        .expect("retitling should succeed");

    assert_eq!(
        decided,
        vec![PieceEvent::Retitled(a_title("The Silent Loom"))]
    );
}

#[test]
fn retitling_to_the_same_title_records_nothing() {
    let decided = a_captured_piece()
        .decide(PieceCommand::Retitle(a_title("The Loom")), &an_author())
        .expect("retitling should succeed");

    assert!(
        decided.is_empty(),
        "an unchanged title must not clutter the history"
    );
}

#[test]
fn a_piece_can_be_given_a_title_it_never_had() {
    let mut piece = a_captured_piece();
    let decided = piece
        .decide(PieceCommand::Retitle(PieceTitle::untitled()), &an_author())
        .expect("clearing a title is a legal change");

    for event in &decided {
        piece.absorb(event, &stamped(2));
    }

    assert!(piece.title().is_untitled());
}

#[test]
fn a_piece_remembers_the_passage_attached_to_it() {
    let mut piece = a_captured_piece();
    let decided = piece
        .decide(
            PieceCommand::AttachPassage(PassageLink::from("passage_9")),
            &an_author(),
        )
        .expect("attaching should succeed");

    for event in &decided {
        piece.absorb(event, &stamped(2));
    }

    assert_eq!(piece.passage(), Some(&PassageLink::from("passage_9")));
}

#[test]
fn a_piece_will_not_take_a_second_passage() {
    let mut piece = a_captured_piece();
    piece.absorb(
        &PieceEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        },
        &stamped(2),
    );

    assert_eq!(
        piece.decide(
            PieceCommand::AttachPassage(PassageLink::from("passage_10")),
            &an_author()
        ),
        Err(PieceError::AlreadyHoldsPassage),
        "a piece must not silently swap the passage its prose lives in"
    );
}

#[test]
fn what_exists_cannot_be_captured_again() {
    assert_eq!(
        a_captured_piece().decide(a_capture(), &an_author()),
        Err(PieceError::AlreadyCaptured)
    );
}

#[test]
fn a_discarded_piece_accepts_nothing_further() {
    let mut piece = a_captured_piece();
    piece.absorb(&PieceEvent::Discarded, &stamped(2));

    assert_eq!(
        piece.decide(PieceCommand::Retitle(a_title("Too late")), &an_author()),
        Err(PieceError::Discarded)
    );
    assert_eq!(
        piece.decide(
            PieceCommand::AttachPassage(PassageLink::from("passage_9")),
            &an_author()
        ),
        Err(PieceError::Discarded)
    );
    assert_eq!(
        piece.decide(PieceCommand::Discard, &an_author()),
        Err(PieceError::Discarded)
    );
}

#[test]
fn a_snapshot_replays_into_exactly_the_piece_it_came_from() {
    let mut piece = a_captured_piece();
    piece.absorb(
        &PieceEvent::Retitled(a_title("The Silent Loom")),
        &stamped(2),
    );
    piece.absorb(
        &PieceEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        },
        &stamped(3),
    );
    piece.absorb(&PieceEvent::Discarded, &stamped(4));

    let snapshot = piece.snapshot();
    let restored = Piece::born(&snapshot, &stamped(5)).expect("a snapshot gives birth");

    assert_eq!(restored, piece);
}

#[test]
fn a_snapshot_declares_itself_a_snapshot() {
    assert!(a_captured_piece().snapshot().is_snapshot());
    assert!(!PieceEvent::Discarded.is_snapshot());
}

#[test]
fn a_stream_that_does_not_start_with_a_capture_gives_birth_to_nothing() {
    assert!(Piece::born(&PieceEvent::Discarded, &stamped(1)).is_none());
    assert!(
        Piece::born(
            &PieceEvent::PassageAttached {
                passage: PassageLink::from("passage_9")
            },
            &stamped(1)
        )
        .is_none()
    );
}

#[test]
fn every_event_has_a_name_of_its_own() {
    use std::collections::HashSet;

    let named: HashSet<_> = [
        PieceEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        }
        .name(),
        PieceEvent::Retitled(a_title("a")).name(),
        PieceEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        }
        .name(),
        PieceEvent::Discarded.name(),
        a_captured_piece().snapshot().name(),
    ]
    .into_iter()
    .collect();

    assert_eq!(
        named.len(),
        5,
        "two events sharing a name could not be told apart in storage"
    );
}

#[test]
fn a_replayed_stream_ends_where_the_events_say() {
    let piece = grown(&[
        PieceEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        },
        PieceEvent::Retitled(a_title("The Silent Loom")),
        PieceEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        },
    ]);

    assert_eq!(piece.title(), &a_title("The Silent Loom"));
    assert_eq!(piece.passage(), Some(&PassageLink::from("passage_9")));
    assert!(!piece.is_discarded());
}
