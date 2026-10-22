use eventsourcing::{AccountId, Agent, Aggregate, AggregateId, Event, EventMetadata, Version};
use time::{Duration, OffsetDateTime};

use crate::details::{BookLanguage, ProjectDescription, ProjectDetails, QuotationMarks};
use crate::name::ProjectName;
use crate::project::*;

fn an_author() -> Agent {
    Agent::Account(AccountId::from("author-7"))
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
    ProjectCommand::Start(a_name("The Weaver's Apprentice"), ProjectDetails::default())
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
        (
            ProjectEvent::QuotationChanged(QuotationMarks::Curly),
            "QUOTATION_CHANGED",
        ),
        (ProjectEvent::Deleted, "DELETED"),
        (a_started_project().snapshot(), "SNAPSHOTTED"),
    ] {
        assert_eq!(event.name().as_str(), named);
    }
}

fn some_details() -> ProjectDetails {
    ProjectDetails {
        language: Some(BookLanguage::new("de").expect("a plain tag")),
        quotation: Some(QuotationMarks::PointingIn),
        description: ProjectDescription::new("A weaver's daughter, and a war.")
            .expect("a plain description"),
    }
}

#[test]
fn a_project_started_with_details_has_them_at_once() {
    assert_eq!(
        Project::begin(
            ProjectCommand::Start(a_name("The Weaver's Apprentice"), some_details()),
            &an_author()
        )
        .expect("starting should succeed"),
        vec![
            ProjectEvent::Started(a_name("The Weaver's Apprentice")),
            ProjectEvent::DetailsChanged(some_details()),
        ]
    );
}

#[test]
fn a_project_started_without_details_is_only_started() {
    assert_eq!(
        Project::begin(a_start(), &an_author()).expect("starting should succeed"),
        vec![ProjectEvent::Started(a_name("The Weaver's Apprentice"))],
        "no details means no event saying so"
    );
}

#[test]
fn changing_a_project_s_details_moves_when_it_was_updated() {
    let mut project = a_started_project();

    let events = project
        .decide(ProjectCommand::ChangeDetails(some_details()), &an_author())
        .expect("changing the details should succeed");
    project.apply(&events[0], &stamped(2, at(2_000)));

    assert_eq!(project.details(), &some_details());
    assert_eq!(project.updated_at(), at(2_000));
}

#[test]
fn changing_a_project_s_details_to_what_they_are_records_nothing() {
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::DetailsChanged(some_details()),
        &stamped(2, at(2_000)),
    );

    assert_eq!(
        project.decide(ProjectCommand::ChangeDetails(some_details()), &an_author()),
        Ok(vec![])
    );
}

fn changed(project: &Project, details: ProjectDetails) -> Vec<ProjectEvent> {
    project
        .decide(ProjectCommand::ChangeDetails(details), &an_author())
        .expect("changing the details should succeed")
}

fn with_details(details: ProjectDetails) -> Project {
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::DetailsChanged(details),
        &stamped(2, at(2_000)),
    );
    project
}

#[test]
fn choosing_another_quotation_style_says_so_apart_from_the_details() {
    let project = with_details(some_details());
    let restyled = ProjectDetails {
        quotation: Some(QuotationMarks::LowHigh),
        ..some_details()
    };

    assert_eq!(
        changed(&project, restyled.clone()),
        vec![
            ProjectEvent::DetailsChanged(restyled),
            ProjectEvent::QuotationChanged(QuotationMarks::LowHigh),
        ]
    );
}

#[test]
fn a_new_language_brings_its_quotation_style_when_none_was_chosen() {
    let project = with_details(ProjectDetails::default());
    let french = ProjectDetails {
        language: Some(BookLanguage::new("fr").expect("a plain tag")),
        ..ProjectDetails::default()
    };

    assert_eq!(
        changed(&project, french.clone()),
        vec![
            ProjectEvent::DetailsChanged(french),
            ProjectEvent::QuotationChanged(QuotationMarks::SpacedOut),
        ]
    );
}

#[test]
fn details_that_leave_the_marks_as_they_look_do_not_restyle_the_book() {
    let project = with_details(some_details());
    let described = ProjectDetails {
        description: ProjectDescription::new("Only the blurb changed.").expect("plain"),
        ..some_details()
    };
    let chosen_as_the_language_would = ProjectDetails {
        quotation: None,
        ..some_details()
    };

    assert_eq!(
        changed(&project, described.clone()),
        vec![ProjectEvent::DetailsChanged(described)]
    );
    assert_eq!(
        changed(&project, chosen_as_the_language_would.clone()),
        vec![ProjectEvent::DetailsChanged(chosen_as_the_language_would)],
        "German quotes »so« whether chosen or not"
    );
}

#[test]
fn a_deleted_project_s_details_cannot_change() {
    let mut project = a_started_project();
    project.apply(&ProjectEvent::Deleted, &stamped(2, at(2_000)));

    assert_eq!(
        project.decide(ProjectCommand::ChangeDetails(some_details()), &an_author()),
        Err(ProjectError::Deleted)
    );
}

#[test]
fn a_project_s_details_survive_its_snapshot() {
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::DetailsChanged(some_details()),
        &stamped(2, at(2_000)),
    );

    let restored = Project::from_first(&project.snapshot(), &stamped(2, at(2_000)))
        .expect("a snapshot gives birth");

    assert_eq!(restored.details(), &some_details());
}
