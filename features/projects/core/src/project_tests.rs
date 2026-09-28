use eventsourcing::{Agent, AgentId, Aggregate, AggregateId, Event, EventMetadata, Version};
use time::{Duration, OffsetDateTime};

use crate::name::ProjectName;
use crate::project::*;

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_name(saying: &str) -> ProjectName {
    ProjectName::new(saying).expect("a plain name is fine")
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn stamped(version: u64, occurred_at: OffsetDateTime) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("project_1"),
        kind: KIND,
        version: Version::of(version),
        agent: an_author(),
        occurred_at,
        is_snapshot: false,
    }
}

fn a_start() -> ProjectCommand {
    ProjectCommand::Start(a_name("The Weaver's Apprentice"))
}

fn a_started_project() -> Project {
    let events = ProjectEvent::Started(a_name("The Weaver's Apprentice"));

    Project::from_first(&events, &stamped(1, at(1_000))).expect("the first event gives birth")
}

#[test]
fn a_project_is_started_with_a_name() {
    let events = ProjectEvent::Started(a_name("The Weaver's Apprentice"));

    assert_eq!(
        Project::begin(a_start(), &an_author()).expect("starting should succeed"),
        vec![events]
    );
}

#[test]
fn nothing_but_starting_can_be_the_first_thing_that_happens() {
    for command in [
        ProjectCommand::Rename(a_name("Renamed")),
        ProjectCommand::Delete,
    ] {
        assert_eq!(
            Project::begin(command, &an_author()),
            Err(ProjectError::NotStartedYet)
        );
    }
}

#[test]
fn a_started_project_is_created_and_updated_at_the_same_moment() {
    let project = a_started_project();

    assert_eq!(project.created_at(), at(1_000));
    assert_eq!(project.updated_at(), at(1_000));
    assert!(!project.is_deleted());
}

#[test]
fn renaming_replaces_the_name_and_moves_updated_at_to_when_it_happened() {
    let mut project = a_started_project();

    let events = project
        .decide(ProjectCommand::Rename(a_name("Renamed")), &an_author())
        .expect("renaming should succeed");
    assert_eq!(events, vec![ProjectEvent::Renamed(a_name("Renamed"))]);

    project.apply(&events[0], &stamped(2, at(2_000)));

    assert_eq!(project.name().as_str(), "Renamed");
    assert_eq!(project.updated_at(), at(2_000));
    assert_eq!(
        project.created_at(),
        at(1_000),
        "renaming leaves the moment the project began untouched"
    );
}

#[test]
fn renaming_to_the_name_it_already_has_happens_not_at_all() {
    let project = a_started_project();

    assert_eq!(
        project
            .decide(
                ProjectCommand::Rename(a_name("The Weaver's Apprentice")),
                &an_author()
            )
            .expect("an unchanged name is not a refusal"),
        vec![],
        "an event saying nothing changed would be a lie in an immutable log"
    );
}

#[test]
fn a_project_cannot_be_started_twice() {
    assert_eq!(
        a_started_project().decide(a_start(), &an_author()),
        Err(ProjectError::AlreadyStarted)
    );
}

#[test]
fn deleting_marks_the_project_without_erasing_what_it_was() {
    let mut project = a_started_project();

    let events = project
        .decide(ProjectCommand::Delete, &an_author())
        .expect("deleting should succeed");
    assert_eq!(events, vec![ProjectEvent::Deleted]);

    project.apply(&events[0], &stamped(2, at(2_000)));

    assert!(project.is_deleted());
    assert_eq!(
        project.name().as_str(),
        "The Weaver's Apprentice",
        "the stream is the audit log, so a deleted project still says what it was called"
    );
}

#[test]
fn a_deleted_project_refuses_everything() {
    let mut project = a_started_project();
    project.apply(&ProjectEvent::Deleted, &stamped(2, at(2_000)));

    for command in [
        ProjectCommand::Rename(a_name("Renamed")),
        ProjectCommand::Delete,
        a_start(),
    ] {
        assert_eq!(
            project.decide(command, &an_author()),
            Err(ProjectError::Deleted)
        );
    }
}

#[test]
fn a_snapshot_carries_everything_replay_would_otherwise_have_to_find() {
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::Renamed(a_name("Renamed")),
        &stamped(2, at(2_000)),
    );

    let snapshot = project.snapshot();

    assert!(snapshot.is_snapshot());
    assert_eq!(
        Project::from_first(&snapshot, &stamped(3, at(9_999))).expect("a snapshot gives birth"),
        project,
        "a snapshot is written long after the fact, so its own timestamp must not become the \
         project's — the moments have to travel in the body"
    );
}

#[test]
fn every_event_says_its_own_name() {
    for (event, named) in [
        (ProjectEvent::Started(a_name("Started")), "STARTED"),
        (ProjectEvent::Renamed(a_name("Renamed")), "RENAMED"),
        (ProjectEvent::Deleted, "DELETED"),
        (a_started_project().snapshot(), "SNAPSHOTTED"),
    ] {
        assert_eq!(event.name().as_str(), named);
    }
}
