use eventsourcing::{Aggregate, AggregateContext, AggregateId, Event, EventMetadata, Version};
use projects_contract::context::PROJECT;
use time::OffsetDateTime;

use crate::details::{BookLanguage, ProjectDescription, ProjectDetails, QuotationMarks};
use crate::name::ProjectName;
use crate::project::*;

fn a_name(saying: &str) -> ProjectName {
    ProjectName::new(saying).expect("a plain name is fine")
}

fn stamped(version: u64, occurred_at: OffsetDateTime) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("project_1"),
        kind: KIND,
        version: Version::of(version),
        agent: eventsourcing::an_author(),
        occurred_at,
        is_snapshot: false,
        context: AggregateContext::default(),
    }
}

fn a_start() -> ProjectCommand {
    ProjectCommand::Start(a_name("The Weaver's Apprentice"), ProjectDetails::default())
}

fn a_started_project() -> Project {
    let events = ProjectEvent::Started(a_name("The Weaver's Apprentice"));

    Project::from_first(&events, &stamped(1, clock::at(1_000)))
        .expect("the first event gives birth")
}

#[test]
fn a_project_is_started_with_a_name() {
    // given: the start it should give
    let events = ProjectEvent::Started(a_name("The Weaver's Apprentice"));

    // expect: starting a project gives just that
    assert_eq!(
        Project::begin(a_start(), &eventsourcing::an_author()).expect("starting should succeed"),
        vec![events]
    );
}

#[test]
fn nothing_but_starting_can_be_the_first_thing_that_happens() {
    // expect: every other command is refused before a start
    for command in [
        ProjectCommand::Rename(a_name("Renamed")),
        ProjectCommand::Delete,
        ProjectCommand::DeleteAbandoned,
    ] {
        assert_eq!(
            Project::begin(command, &eventsourcing::an_author()),
            Err(ProjectError::NotStartedYet)
        );
    }
}

#[test]
fn a_started_project_is_created_and_updated_at_the_same_moment() {
    // given: a started project
    let project = a_started_project();

    // then: it was created and updated as it started
    assert_eq!(project.created_at(), clock::at(1_000));
    assert_eq!(project.updated_at(), clock::at(1_000));
    // and: it is not deleted
    assert!(!project.is_deleted());
}

#[test]
fn renaming_replaces_the_name_and_moves_updated_at_to_when_it_happened() {
    // given: a started project
    let mut project = a_started_project();

    // when: it is renamed
    let events = project
        .decide(
            ProjectCommand::Rename(a_name("Renamed")),
            &eventsourcing::an_author(),
        )
        .expect("renaming should succeed");
    // then: it says it was renamed
    assert_eq!(events, vec![ProjectEvent::Renamed(a_name("Renamed"))]);

    // when: the renaming happens later
    project.apply(&events[0], &stamped(2, clock::at(2_000)));

    // then: it has the new name, updated as of then
    assert_eq!(project.name().as_str(), "Renamed");
    assert_eq!(project.updated_at(), clock::at(2_000));
    // and: it was still created when it started
    assert_eq!(
        project.created_at(),
        clock::at(1_000),
        "renaming leaves the moment the project began untouched"
    );
}

#[test]
fn renaming_to_the_name_it_already_has_happens_not_at_all() {
    // given: a started project
    let project = a_started_project();

    // expect: renaming it to its own name changes nothing
    assert_eq!(
        project
            .decide(
                ProjectCommand::Rename(a_name("The Weaver's Apprentice")),
                &eventsourcing::an_author()
            )
            .expect("an unchanged name is not a refusal"),
        vec![],
        "an event saying nothing changed would be a lie in an immutable log"
    );
}

#[test]
fn a_project_cannot_be_started_twice() {
    // expect: a started project refuses another start
    assert_eq!(
        a_started_project().decide(a_start(), &eventsourcing::an_author()),
        Err(ProjectError::AlreadyStarted)
    );
}

fn a_trashed_project() -> Project {
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::Trashed {
            until: clock::at(9_000),
        },
        &stamped(2, clock::at(2_000)),
    );

    project
}

#[test]
fn deleting_marks_the_project_without_erasing_what_it_was() {
    // given: a project in the trash
    let mut project = a_trashed_project();

    // when: it is deleted
    let events = project
        .decide(ProjectCommand::Delete, &eventsourcing::an_author())
        .expect("deleting should succeed");
    // then: it says it was deleted
    assert_eq!(events, vec![ProjectEvent::Deleted]);

    // when: the deleting happens
    project.apply(&events[0], &stamped(3, clock::at(3_000)));

    // then: it is deleted
    assert!(project.is_deleted());
    // and: it still has its name
    assert_eq!(
        project.name().as_str(),
        "The Weaver's Apprentice",
        "the stream is the audit log, so a deleted project still says what it was called"
    );
}

#[test]
fn a_deleted_project_refuses_everything() {
    // given: a deleted project
    let mut project = a_started_project();
    project.apply(&ProjectEvent::Deleted, &stamped(2, clock::at(2_000)));

    // expect: every command is refused
    for command in [
        ProjectCommand::Rename(a_name("Renamed")),
        ProjectCommand::Delete,
        ProjectCommand::DeleteAbandoned,
        a_start(),
    ] {
        assert_eq!(
            project.decide(command, &eventsourcing::an_author()),
            Err(ProjectError::Deleted)
        );
    }
}

#[test]
fn a_snapshot_carries_everything_replay_would_otherwise_have_to_find() {
    // given: a renamed project
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::Renamed(a_name("Renamed")),
        &stamped(2, clock::at(2_000)),
    );

    // when: it is snapshotted
    let snapshot = project.snapshot();

    // then: the snapshot says it is one
    assert!(snapshot.is_snapshot());
    // and: a project born of it, however late, is the same project
    assert_eq!(
        Project::from_first(&snapshot, &stamped(3, clock::at(9_999)))
            .expect("a snapshot gives birth"),
        project,
        "a snapshot is written long after the fact, so its own timestamp must not become the \
         project's — the moments have to travel in the body"
    );
}

#[test]
fn every_event_says_its_own_name() {
    // expect: each event is named for what it is
    for (event, named) in [
        (ProjectEvent::Started(a_name("Started")), "STARTED"),
        (ProjectEvent::Renamed(a_name("Renamed")), "RENAMED"),
        (
            ProjectEvent::QuotationChanged(QuotationMarks::Curly),
            "QUOTATION_CHANGED",
        ),
        (
            ProjectEvent::Trashed {
                until: clock::at(9_000),
            },
            "TRASHED",
        ),
        (ProjectEvent::Restored, "RESTORED"),
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
    // expect: starting with details gives the start and the details
    assert_eq!(
        Project::begin(
            ProjectCommand::Start(a_name("The Weaver's Apprentice"), some_details()),
            &eventsourcing::an_author()
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
    // expect: starting without details gives the start alone
    assert_eq!(
        Project::begin(a_start(), &eventsourcing::an_author()).expect("starting should succeed"),
        vec![ProjectEvent::Started(a_name("The Weaver's Apprentice"))],
        "no details means no event saying so"
    );
}

#[test]
fn changing_a_project_s_details_moves_when_it_was_updated() {
    // given: a started project
    let mut project = a_started_project();

    // when: its details are changed later
    let events = project
        .decide(
            ProjectCommand::ChangeDetails(some_details()),
            &eventsourcing::an_author(),
        )
        .expect("changing the details should succeed");
    project.apply(&events[0], &stamped(2, clock::at(2_000)));

    // then: it has the new details
    assert_eq!(project.details(), &some_details());
    // and: it was updated as of then
    assert_eq!(project.updated_at(), clock::at(2_000));
}

#[test]
fn changing_a_project_s_details_to_what_they_are_records_nothing() {
    // given: a project with details
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::DetailsChanged(some_details()),
        &stamped(2, clock::at(2_000)),
    );

    // expect: changing them to the same details records nothing
    assert_eq!(
        project.decide(
            ProjectCommand::ChangeDetails(some_details()),
            &eventsourcing::an_author()
        ),
        Ok(vec![])
    );
}

fn changing_details(project: &Project, details: ProjectDetails) -> Vec<ProjectEvent> {
    project
        .decide(
            ProjectCommand::ChangeDetails(details),
            &eventsourcing::an_author(),
        )
        .expect("changing the details should succeed")
}

fn a_project_with(details: ProjectDetails) -> Project {
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::DetailsChanged(details),
        &stamped(2, clock::at(2_000)),
    );
    project
}

#[test]
fn choosing_another_quotation_style_says_so_apart_from_the_details() {
    // given: a project with details
    let project = a_project_with(some_details());
    // and: the same details with another quotation style
    let restyled = ProjectDetails {
        quotation: Some(QuotationMarks::LowHigh),
        ..some_details()
    };

    // expect: changing to them says the details and the quotation style changed
    assert_eq!(
        changing_details(&project, restyled.clone()),
        vec![
            ProjectEvent::DetailsChanged(restyled),
            ProjectEvent::QuotationChanged(QuotationMarks::LowHigh),
        ]
    );
}

#[test]
fn a_new_language_brings_its_quotation_style_when_none_was_chosen() {
    // given: a project with no details chosen
    let project = a_project_with(ProjectDetails::default());
    // and: details with only a language
    let french = ProjectDetails {
        language: Some(BookLanguage::new("fr").expect("a plain tag")),
        ..ProjectDetails::default()
    };

    // expect: changing to them brings the language's quotation style
    assert_eq!(
        changing_details(&project, french.clone()),
        vec![
            ProjectEvent::DetailsChanged(french),
            ProjectEvent::QuotationChanged(QuotationMarks::SpacedOut),
        ]
    );
}

#[test]
fn details_that_leave_the_marks_as_they_look_do_not_restyle_the_book() {
    // given: a project with details
    let project = a_project_with(some_details());
    // and: the same details with another description
    let described = ProjectDetails {
        description: ProjectDescription::new("Only the blurb changed.").expect("plain"),
        ..some_details()
    };
    // and: the same details leaving the quotation style to the language
    let chosen_as_the_language_would = ProjectDetails {
        quotation: None,
        ..some_details()
    };

    // expect: changing to another description changes only the details
    assert_eq!(
        changing_details(&project, described.clone()),
        vec![ProjectEvent::DetailsChanged(described)]
    );
    // and: leaving the style to the language that would choose it changes only the details
    assert_eq!(
        changing_details(&project, chosen_as_the_language_would.clone()),
        vec![ProjectEvent::DetailsChanged(chosen_as_the_language_would)],
        "German quotes »so« whether chosen or not"
    );
}

#[test]
fn a_deleted_project_s_details_cannot_change() {
    // given: a deleted project
    let mut project = a_started_project();
    project.apply(&ProjectEvent::Deleted, &stamped(2, clock::at(2_000)));

    // expect: changing its details is refused
    assert_eq!(
        project.decide(
            ProjectCommand::ChangeDetails(some_details()),
            &eventsourcing::an_author()
        ),
        Err(ProjectError::Deleted)
    );
}

#[test]
fn a_project_s_details_survive_its_snapshot() {
    // given: a project with details
    let mut project = a_started_project();
    project.apply(
        &ProjectEvent::DetailsChanged(some_details()),
        &stamped(2, clock::at(2_000)),
    );

    // when: it is restored from its snapshot
    let restored = Project::from_first(&project.snapshot(), &stamped(2, clock::at(2_000)))
        .expect("a snapshot gives birth");

    // then: it has its details
    assert_eq!(restored.details(), &some_details());
}

#[test]
fn a_project_is_its_own_context_and_stays_so_through_a_snapshot() {
    // given: a started project
    let project = a_started_project();

    // when: it is rebuilt from its snapshot
    let rebuilt = Project::from_first(&project.snapshot(), &stamped(9, clock::at(5_000)))
        .expect("a snapshot gives birth");

    // then: both are their own context
    assert_eq!(project.context().get(PROJECT), Some("project_1"));
    assert_eq!(rebuilt.context().get(PROJECT), Some("project_1"));
}

#[test]
fn trashing_keeps_the_project_until_the_moment_it_is_told() {
    // given: a started project
    let mut project = a_started_project();

    // when: it is trashed until a moment
    let events = project
        .decide(
            ProjectCommand::Trash {
                until: clock::at(9_000),
            },
            &eventsourcing::an_author(),
        )
        .expect("trashing should succeed");
    // then: it says it was trashed until then
    assert_eq!(
        events,
        vec![ProjectEvent::Trashed {
            until: clock::at(9_000)
        }]
    );

    // when: the trashing happens
    project.apply(&events[0], &stamped(2, clock::at(2_000)));

    // then: it is in the trash until that moment
    assert!(project.is_trashed());
    assert_eq!(project.trashed_until(), Some(clock::at(9_000)));
    // and: it is not deleted
    assert!(!project.is_deleted(), "in the trash is not deleted yet");
}

#[test]
fn a_project_in_the_trash_takes_no_change_and_is_not_trashed_twice() {
    // given: a project in the trash
    let project = a_trashed_project();

    // expect: every change is refused
    for command in [
        ProjectCommand::Rename(a_name("Renamed")),
        ProjectCommand::ChangeDetails(some_details()),
    ] {
        assert_eq!(
            project.decide(command, &eventsourcing::an_author()),
            Err(ProjectError::Trashed)
        );
    }
    // and: trashing it again is refused
    assert_eq!(
        project.decide(
            ProjectCommand::Trash {
                until: clock::at(99_000)
            },
            &eventsourcing::an_author()
        ),
        Err(ProjectError::AlreadyTrashed),
        "trashing again would move when it is purged"
    );
}

#[test]
fn restoring_takes_the_project_out_of_the_trash_as_it_was() {
    // given: a project in the trash
    let mut project = a_trashed_project();

    // when: it is restored
    let events = project
        .decide(ProjectCommand::Restore, &eventsourcing::an_author())
        .expect("restoring should succeed");
    // then: it says it was restored
    assert_eq!(events, vec![ProjectEvent::Restored]);

    // when: the restoring happens
    project.apply(&events[0], &stamped(3, clock::at(3_000)));

    // then: it is out of the trash
    assert!(!project.is_trashed());
    assert_eq!(project.trashed_until(), None);
    // and: it takes changes again
    assert_eq!(
        project.decide(
            ProjectCommand::Rename(a_name("Renamed")),
            &eventsourcing::an_author()
        ),
        Ok(vec![ProjectEvent::Renamed(a_name("Renamed"))]),
        "a project restored takes changes again"
    );
}

#[test]
fn only_a_project_in_the_trash_is_restored_or_deleted() {
    // given: a project out of the trash
    let project = a_started_project();

    // expect: restoring and deleting it are refused
    for command in [ProjectCommand::Restore, ProjectCommand::Delete] {
        assert_eq!(
            project.decide(command, &eventsourcing::an_author()),
            Err(ProjectError::NotTrashed),
            "a project is deleted only once its time in the trash is over"
        );
    }
}

#[test]
fn a_project_nobody_is_left_in_is_deleted_in_the_trash_or_out_of_it() {
    // expect: a project out of the trash and one in it are both deleted
    for project in [a_started_project(), a_trashed_project()] {
        assert_eq!(
            project.decide(ProjectCommand::DeleteAbandoned, &eventsourcing::an_author()),
            Ok(vec![ProjectEvent::Deleted]),
            "a project nobody is left in has nobody to keep it"
        );
    }
}

#[test]
fn a_snapshot_of_a_project_in_the_trash_keeps_until_when() {
    // given: a project in the trash
    let project = a_trashed_project();

    // expect: a project born of its snapshot is in the trash until the same moment
    assert_eq!(
        Project::from_first(&project.snapshot(), &stamped(3, clock::at(9_999)))
            .expect("a snapshot gives birth")
            .trashed_until(),
        Some(clock::at(9_000))
    );
}
