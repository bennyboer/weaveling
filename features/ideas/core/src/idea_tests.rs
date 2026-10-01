use eventsourcing::{Agent, AgentId, Aggregate, AggregateId, Event, EventMetadata, Version};
use time::OffsetDateTime;

use crate::idea::*;
use crate::title::IdeaTitle;

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_title(saying: &str) -> IdeaTitle {
    IdeaTitle::new(saying).expect("a plain title is fine")
}

fn stamped(version: u64) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("idea_1"),
        kind: KIND,
        version: Version::of(version),
        agent: an_author(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        is_snapshot: false,
    }
}

fn a_capture() -> IdeaCommand {
    IdeaCommand::Capture {
        project: ProjectLink::from("project_1"),
        title: a_title("The Loom"),
    }
}

fn a_captured_idea() -> Idea {
    let events = Idea::begin(a_capture(), &an_author()).expect("capturing should succeed");

    grown(&events)
}

fn grown(events: &[IdeaEvent]) -> Idea {
    let (first, rest) = events.split_first().expect("at least one event");
    let mut idea = Idea::from_first(first, &stamped(1)).expect("the first event gives birth");

    for (counted, event) in rest.iter().enumerate() {
        idea.apply(event, &stamped(counted as u64 + 2));
    }

    idea
}

#[test]
fn a_idea_is_captured_with_a_project_and_a_title() {
    let events = Idea::begin(a_capture(), &an_author()).expect("capturing should succeed");

    assert_eq!(
        events,
        vec![IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        }]
    );
}

#[test]
fn a_idea_may_be_captured_with_no_title_at_all() {
    let events = Idea::begin(
        IdeaCommand::Capture {
            project: ProjectLink::from("project_1"),
            title: IdeaTitle::untitled(),
        },
        &an_author(),
    )
    .expect("an idea arrives before its name");

    assert_eq!(grown(&events).title(), &IdeaTitle::untitled());
}

#[test]
fn nothing_but_a_capture_can_start_a_idea() {
    assert_eq!(
        Idea::begin(IdeaCommand::Discard, &an_author()),
        Err(IdeaError::NotCapturedYet)
    );
    assert_eq!(
        Idea::begin(IdeaCommand::Retitle(a_title("Too soon")), &an_author()),
        Err(IdeaError::NotCapturedYet)
    );
}

#[test]
fn a_captured_idea_holds_no_passage_yet() {
    let idea = a_captured_idea();

    assert_eq!(idea.passage(), None, "a passage is attached on first write");
    assert!(!idea.is_discarded());
    assert_eq!(idea.project(), &ProjectLink::from("project_1"));
}

#[test]
fn retitling_records_the_title_it_became() {
    let decided = a_captured_idea()
        .decide(
            IdeaCommand::Retitle(a_title("The Silent Loom")),
            &an_author(),
        )
        .expect("retitling should succeed");

    assert_eq!(
        decided,
        vec![IdeaEvent::Retitled(a_title("The Silent Loom"))]
    );
}

#[test]
fn retitling_to_the_same_title_records_nothing() {
    let decided = a_captured_idea()
        .decide(IdeaCommand::Retitle(a_title("The Loom")), &an_author())
        .expect("retitling should succeed");

    assert!(
        decided.is_empty(),
        "an unchanged title must not clutter the history"
    );
}

#[test]
fn a_idea_can_be_given_a_title_it_never_had() {
    let mut idea = a_captured_idea();
    let decided = idea
        .decide(IdeaCommand::Retitle(IdeaTitle::untitled()), &an_author())
        .expect("clearing a title is a legal change");

    for event in &decided {
        idea.apply(event, &stamped(2));
    }

    assert!(idea.title().is_untitled());
}

#[test]
fn a_idea_remembers_the_passage_attached_to_it() {
    let mut idea = a_captured_idea();
    let decided = idea
        .decide(
            IdeaCommand::AttachPassage(PassageLink::from("passage_9")),
            &an_author(),
        )
        .expect("attaching should succeed");

    for event in &decided {
        idea.apply(event, &stamped(2));
    }

    assert_eq!(idea.passage(), Some(&PassageLink::from("passage_9")));
}

#[test]
fn a_idea_will_not_take_a_second_passage() {
    let mut idea = a_captured_idea();
    idea.apply(
        &IdeaEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        },
        &stamped(2),
    );

    assert_eq!(
        idea.decide(
            IdeaCommand::AttachPassage(PassageLink::from("passage_10")),
            &an_author()
        ),
        Err(IdeaError::AlreadyHoldsPassage),
        "a idea must not silently swap the passage its prose lives in"
    );
}

#[test]
fn what_exists_cannot_be_captured_again() {
    assert_eq!(
        a_captured_idea().decide(a_capture(), &an_author()),
        Err(IdeaError::AlreadyCaptured)
    );
}

#[test]
fn a_discarded_idea_accepts_nothing_further() {
    let mut idea = a_captured_idea();
    idea.apply(&IdeaEvent::Discarded { passage: None }, &stamped(2));

    assert_eq!(
        idea.decide(IdeaCommand::Retitle(a_title("Too late")), &an_author()),
        Err(IdeaError::Discarded)
    );
    assert_eq!(
        idea.decide(
            IdeaCommand::AttachPassage(PassageLink::from("passage_9")),
            &an_author()
        ),
        Err(IdeaError::Discarded)
    );
    assert_eq!(
        idea.decide(IdeaCommand::Discard, &an_author()),
        Err(IdeaError::Discarded)
    );
}

#[test]
fn a_snapshot_replays_into_exactly_the_idea_it_came_from() {
    let mut idea = a_captured_idea();
    idea.apply(
        &IdeaEvent::Retitled(a_title("The Silent Loom")),
        &stamped(2),
    );
    idea.apply(
        &IdeaEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        },
        &stamped(3),
    );
    idea.apply(&IdeaEvent::Discarded { passage: None }, &stamped(4));

    let snapshot = idea.snapshot();
    let restored = Idea::from_first(&snapshot, &stamped(5)).expect("a snapshot gives birth");

    assert_eq!(restored, idea);
}

#[test]
fn a_snapshot_declares_itself_a_snapshot() {
    assert!(a_captured_idea().snapshot().is_snapshot());
    assert!(!IdeaEvent::Discarded { passage: None }.is_snapshot());
}

#[test]
fn a_stream_that_does_not_start_with_a_capture_gives_birth_to_nothing() {
    assert!(Idea::from_first(&IdeaEvent::Discarded { passage: None }, &stamped(1)).is_none());
    assert!(
        Idea::from_first(
            &IdeaEvent::PassageAttached {
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
        IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        }
        .name(),
        IdeaEvent::Retitled(a_title("a")).name(),
        IdeaEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        }
        .name(),
        IdeaEvent::Discarded { passage: None }.name(),
        a_captured_idea().snapshot().name(),
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
    let idea = grown(&[
        IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        },
        IdeaEvent::Retitled(a_title("The Silent Loom")),
        IdeaEvent::PassageAttached {
            passage: PassageLink::from("passage_9"),
        },
    ]);

    assert_eq!(idea.title(), &a_title("The Silent Loom"));
    assert_eq!(idea.passage(), Some(&PassageLink::from("passage_9")));
    assert!(!idea.is_discarded());
}

#[test]
fn discarding_a_idea_says_which_passage_went_with_it() {
    let mut idea = a_captured_idea();
    idea.apply(
        &IdeaEvent::PassageAttached {
            passage: PassageLink::from("passage_1"),
        },
        &stamped(2),
    );

    let events = idea
        .decide(IdeaCommand::Discard, &an_author())
        .expect("discarding should succeed");

    assert_eq!(
        events,
        vec![IdeaEvent::Discarded {
            passage: Some(PassageLink::from("passage_1")),
        }],
        "passages cannot look a idea up across the seam, so the discard has to carry the          passage or the prose is orphaned with nothing to say so"
    );
}

#[test]
fn discarding_a_idea_that_never_had_prose_carries_no_passage() {
    let events = a_captured_idea()
        .decide(IdeaCommand::Discard, &an_author())
        .expect("discarding should succeed");

    assert_eq!(events, vec![IdeaEvent::Discarded { passage: None }]);
}
