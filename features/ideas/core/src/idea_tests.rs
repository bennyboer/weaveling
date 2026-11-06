use eventsourcing::{Aggregate, AggregateContext, AggregateId, Event, EventMetadata, Version};
use projects_contract::context::PROJECT;
use time::OffsetDateTime;

use crate::idea::*;
use crate::title::IdeaTitle;

fn a_title(saying: &str) -> IdeaTitle {
    IdeaTitle::new(saying).expect("a plain title is fine")
}

fn stamped(version: u64) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("idea_1"),
        kind: KIND,
        version: Version::of(version),
        agent: eventsourcing::an_author(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        is_snapshot: false,
        context: AggregateContext::default(),
    }
}

fn a_capture() -> IdeaCommand {
    IdeaCommand::Capture {
        project: ProjectLink::from("project_1"),
        title: a_title("The Loom"),
    }
}

fn a_captured_idea() -> Idea {
    let events =
        Idea::begin(a_capture(), &eventsourcing::an_author()).expect("capturing should succeed");

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
fn an_idea_is_captured_with_a_project_and_a_title() {
    // when: an idea is captured
    let events =
        Idea::begin(a_capture(), &eventsourcing::an_author()).expect("capturing should succeed");

    // then: its project and title are recorded
    assert_eq!(
        events,
        vec![IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        }]
    );
}

#[test]
fn an_idea_may_be_captured_with_no_title_at_all() {
    // when: an idea is captured with no title
    let events = Idea::begin(
        IdeaCommand::Capture {
            project: ProjectLink::from("project_1"),
            title: IdeaTitle::untitled(),
        },
        &eventsourcing::an_author(),
    )
    .expect("an idea arrives before its name");

    // then: it stands untitled
    assert_eq!(grown(&events).title(), &IdeaTitle::untitled());
}

#[test]
fn nothing_but_a_capture_can_start_an_idea() {
    // expect: anything but a capture is refused before there is an idea
    assert_eq!(
        Idea::begin(IdeaCommand::Discard, &eventsourcing::an_author()),
        Err(IdeaError::NotCapturedYet)
    );
    assert_eq!(
        Idea::begin(
            IdeaCommand::Retitle(a_title("Too soon")),
            &eventsourcing::an_author()
        ),
        Err(IdeaError::NotCapturedYet)
    );
}

#[test]
fn retitling_records_the_title_it_became() {
    // when: a captured idea is retitled
    let decided = a_captured_idea()
        .decide(
            IdeaCommand::Retitle(a_title("The Silent Loom")),
            &eventsourcing::an_author(),
        )
        .expect("retitling should succeed");

    // then: the new title is recorded
    assert_eq!(
        decided,
        vec![IdeaEvent::Retitled(a_title("The Silent Loom"))]
    );
}

#[test]
fn retitling_to_the_same_title_records_nothing() {
    // when: a captured idea is retitled to the title it has
    let decided = a_captured_idea()
        .decide(
            IdeaCommand::Retitle(a_title("The Loom")),
            &eventsourcing::an_author(),
        )
        .expect("retitling should succeed");

    // then: nothing is recorded
    assert!(
        decided.is_empty(),
        "an unchanged title must not clutter the history"
    );
}

#[test]
fn an_idea_can_be_given_a_title_it_never_had() {
    // given: a captured idea
    let mut idea = a_captured_idea();
    // when: its title is cleared
    let decided = idea
        .decide(
            IdeaCommand::Retitle(IdeaTitle::untitled()),
            &eventsourcing::an_author(),
        )
        .expect("clearing a title is a legal change");

    for event in &decided {
        idea.apply(event, &stamped(2));
    }

    // then: it is untitled
    assert!(idea.title().is_untitled());
}

#[test]
fn what_exists_cannot_be_captured_again() {
    // expect: a captured idea cannot be captured again
    assert_eq!(
        a_captured_idea().decide(a_capture(), &eventsourcing::an_author()),
        Err(IdeaError::AlreadyCaptured)
    );
}

#[test]
fn a_discarded_idea_accepts_nothing_further() {
    // given: a discarded idea
    let mut idea = a_captured_idea();
    idea.apply(&IdeaEvent::Discarded, &stamped(2));

    // then: it refuses a retitling and a discarding
    assert_eq!(
        idea.decide(
            IdeaCommand::Retitle(a_title("Too late")),
            &eventsourcing::an_author()
        ),
        Err(IdeaError::Discarded)
    );
    assert_eq!(
        idea.decide(IdeaCommand::Discard, &eventsourcing::an_author()),
        Err(IdeaError::Discarded)
    );
}

#[test]
fn a_snapshot_replays_into_exactly_the_idea_it_came_from() {
    // given: an idea that was retitled and discarded
    let mut idea = a_captured_idea();
    idea.apply(
        &IdeaEvent::Retitled(a_title("The Silent Loom")),
        &stamped(2),
    );
    idea.apply(&IdeaEvent::Discarded, &stamped(4));

    // when: it is restored from its snapshot
    let snapshot = idea.snapshot();
    let restored = Idea::from_first(&snapshot, &stamped(5)).expect("a snapshot gives birth");

    // then: it is the idea it came from
    assert_eq!(restored, idea);
}

#[test]
fn a_snapshot_declares_itself_a_snapshot() {
    // expect: a snapshot says it is one and an event does not
    assert!(a_captured_idea().snapshot().is_snapshot());
    assert!(!IdeaEvent::Discarded.is_snapshot());
}

#[test]
fn a_stream_that_does_not_start_with_a_capture_gives_birth_to_nothing() {
    // expect: a stream starting with a discarding gives birth to nothing
    assert!(Idea::from_first(&IdeaEvent::Discarded, &stamped(1)).is_none());
}

#[test]
fn every_event_has_a_name_of_its_own() {
    // given: the names of every kind of event
    use std::collections::HashSet;

    let named: HashSet<_> = [
        IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        }
        .name(),
        IdeaEvent::Retitled(a_title("a")).name(),
        IdeaEvent::Discarded.name(),
        a_captured_idea().snapshot().name(),
    ]
    .into_iter()
    .collect();

    // then: no two are the same
    assert_eq!(
        named.len(),
        4,
        "two events sharing a name could not be told apart in storage"
    );
}

#[test]
fn a_replayed_stream_ends_where_the_events_say() {
    // when: a capture and a retitling are replayed
    let idea = grown(&[
        IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title("The Loom"),
        },
        IdeaEvent::Retitled(a_title("The Silent Loom")),
    ]);

    // then: the idea has the later title
    assert_eq!(idea.title(), &a_title("The Silent Loom"));
    // and: it is not discarded
    assert!(!idea.is_discarded());
}

#[test]
fn an_idea_is_in_the_context_of_its_project() {
    // expect: a captured idea is in the context of its project
    assert_eq!(a_captured_idea().context().get(PROJECT), Some("project_1"));
}
