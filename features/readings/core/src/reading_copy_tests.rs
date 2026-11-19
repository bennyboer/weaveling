use eventsourcing::{Agent, Aggregate, AggregateContext, AggregateId, EventMetadata, Version};
use projects_contract::context::PROJECT;
use time::{Duration, OffsetDateTime};

use crate::link::ProjectLink;
use crate::reading_copies::Extent;
use crate::reading_copy::*;
use crate::reading_copy_name::ReadingCopyName;

const MADE_AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

fn a_name(saying: &str) -> ReadingCopyName {
    ReadingCopyName::new(saying).expect("a plain name is fine")
}

fn days(count: i64) -> Option<OffsetDateTime> {
    Some(MADE_AT + Duration::days(count))
}

fn times(open_until: Option<OffsetDateTime>, deleted_on: Option<OffsetDateTime>) -> Times {
    Times {
        open_until,
        deleted_on,
    }
}

fn hedy() -> Agent {
    Agent::Account(eventsourcing::AccountId::from("account_hedy"))
}

fn stamped(version: u64) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("copy_1"),
        kind: KIND,
        version: Version::of(version),
        agent: hedy(),
        occurred_at: MADE_AT,
        is_snapshot: false,
        context: AggregateContext::default(),
    }
}

fn a_making(of: Times) -> ReadingCopyCommand {
    ReadingCopyCommand::Make {
        project: ProjectLink::from("project_loom"),
        name: a_name("For Lena"),
        extent: Extent::Sections(3),
        times: of,
    }
}

fn grown(events: &[ReadingCopyEvent]) -> ReadingCopy {
    let (first, rest) = events.split_first().expect("at least one event");
    let mut copy = ReadingCopy::from_first(first, &stamped(1)).expect("the first event makes it");

    for (counted, event) in rest.iter().enumerate() {
        copy.apply(event, &stamped(counted as u64 + 2));
    }

    copy
}

/// A copy open for 30 days and deleted after 90.
fn a_copy() -> ReadingCopy {
    grown(
        &ReadingCopy::begin(a_making(times(days(30), days(90))), &hedy())
            .expect("making should succeed"),
    )
}

fn decided(
    copy: &ReadingCopy,
    command: ReadingCopyCommand,
) -> Result<ReadingCopy, ReadingCopyError> {
    let events = copy.decide(command, &hedy())?;
    let mut changed = copy.clone();
    for event in &events {
        changed.apply(event, &stamped(2));
    }

    Ok(changed)
}

#[test]
fn a_copy_is_made_of_a_project_under_a_name_by_whoever_made_it() {
    // when: a copy is made
    let copy = a_copy();

    // then: it holds what it was made with, and who made it and when
    assert_eq!(copy.project(), &ProjectLink::from("project_loom"));
    assert_eq!(copy.name(), &a_name("For Lena"));
    assert_eq!(copy.extent(), Extent::Sections(3));
    assert_eq!(copy.times(), times(days(30), days(90)));
    assert_eq!(copy.made_by(), &hedy());
    assert_eq!(copy.made_at(), MADE_AT);
    assert!(!copy.is_deleted());
    // and: its events carry its project, as the other features find a copy's project by them
    assert_eq!(copy.context().get(PROJECT), Some("project_loom"));
}

#[test]
fn a_copy_cannot_be_made_open_beyond_its_deletion() {
    // expect: open after it is deleted, or with no end though it is deleted, is refused
    for open_too_long in [times(days(91), days(90)), times(None, days(90))] {
        assert_eq!(
            ReadingCopy::begin(a_making(open_too_long), &hedy()),
            Err(ReadingCopyError::OpenBeyondDeletion),
            "{open_too_long:?}"
        );
    }
    // and: open with no end and kept for good, or open until it is deleted, is made
    for holding in [
        times(None, None),
        times(days(90), days(90)),
        times(days(30), None),
    ] {
        assert!(
            ReadingCopy::begin(a_making(holding), &hedy()).is_ok(),
            "{holding:?}"
        );
    }
}

#[test]
fn nothing_but_making_starts_a_copy_and_a_copy_is_made_once() {
    // expect: renaming a copy never made is refused, and so is making one twice
    assert_eq!(
        ReadingCopy::begin(ReadingCopyCommand::Rename(a_name("Too soon")), &hedy()),
        Err(ReadingCopyError::NotMadeYet)
    );
    assert_eq!(
        a_copy().decide(a_making(times(None, None)), &hedy()),
        Err(ReadingCopyError::AlreadyMade)
    );
}

#[test]
fn a_copy_is_renamed_and_renaming_it_as_it_is_changes_nothing() {
    // given: a copy
    let copy = a_copy();

    // expect: a new name is taken, and its own name tells nothing
    assert_eq!(
        decided(&copy, ReadingCopyCommand::Rename(a_name("For Lena, again")))
            .expect("renaming should succeed")
            .name(),
        &a_name("For Lena, again")
    );
    assert_eq!(
        copy.decide(ReadingCopyCommand::Rename(a_name("For Lena")), &hedy()),
        Ok(vec![])
    );
}

#[test]
fn a_copy_s_times_move_later_and_to_no_end() {
    // given: a copy open for 30 days and deleted after 90
    let copy = a_copy();

    // expect: each time may move later, the deletion first so the opening may follow
    for later in [
        times(days(60), days(90)),
        times(days(30), days(365)),
        times(days(365), days(365)),
        times(days(30), None),
        times(None, None),
    ] {
        assert_eq!(
            decided(&copy, ReadingCopyCommand::Extend(later))
                .expect("extending should succeed")
                .times(),
            later
        );
    }
}

#[test]
fn a_copy_s_times_never_move_earlier_nor_give_an_end_where_there_was_none() {
    // given: a copy open for 30 days and deleted after 90, and one kept for good with no end
    let copy = a_copy();
    let endless = decided(&copy, ReadingCopyCommand::Extend(times(None, None)))
        .expect("extending should succeed");

    // expect: an earlier opening or deletion is refused
    for earlier in [times(days(20), days(90)), times(days(30), days(60))] {
        assert_eq!(
            copy.decide(ReadingCopyCommand::Extend(earlier), &hedy()),
            Err(ReadingCopyError::Earlier),
            "{earlier:?}"
        );
    }
    // and: an end where there was none is refused too
    assert_eq!(
        endless.decide(ReadingCopyCommand::Extend(times(days(400), None)), &hedy()),
        Err(ReadingCopyError::Earlier)
    );
}

#[test]
fn an_extended_copy_still_never_stays_open_beyond_its_deletion() {
    // given: a copy open for 30 days and deleted after 90
    let copy = a_copy();

    // expect: opening it beyond its deletion, or with no end while it is deleted, is refused
    for too_long in [times(days(120), days(90)), times(None, days(90))] {
        assert_eq!(
            copy.decide(ReadingCopyCommand::Extend(too_long), &hedy()),
            Err(ReadingCopyError::OpenBeyondDeletion),
            "{too_long:?}"
        );
    }
    // and: extending it to the times it has changes nothing
    assert_eq!(
        copy.decide(
            ReadingCopyCommand::Extend(times(days(30), days(90))),
            &hedy()
        ),
        Ok(vec![])
    );
}

#[test]
fn a_deleted_copy_accepts_no_changes() {
    // given: a deleted copy
    let deleted = decided(&a_copy(), ReadingCopyCommand::Delete).expect("deleting should succeed");
    assert!(deleted.is_deleted());

    // expect: renaming, extending and deleting it again are refused
    for command in [
        ReadingCopyCommand::Rename(a_name("Too late")),
        ReadingCopyCommand::Extend(times(None, None)),
        ReadingCopyCommand::Delete,
    ] {
        assert_eq!(
            deleted.decide(command, &hedy()),
            Err(ReadingCopyError::Deleted)
        );
    }
}

#[test]
fn a_copy_comes_back_from_its_snapshot_as_it_was() {
    // given: a copy renamed, extended and deleted
    let mut copy = a_copy();
    for command in [
        ReadingCopyCommand::Rename(a_name("For Lena, again")),
        ReadingCopyCommand::Extend(times(days(60), None)),
        ReadingCopyCommand::Delete,
    ] {
        copy = decided(&copy, command).expect("each change should succeed");
    }

    // when: it is grown again from its snapshot
    let again =
        ReadingCopy::from_first(&copy.snapshot(), &stamped(9)).expect("a snapshot starts it");

    // then: it is as it was, its maker and when it was made included
    assert_eq!(again, copy);
}
