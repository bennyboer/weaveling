use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use members_contract::MembersEventDTO;
use projects_contract::ProjectEventDTO;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use quotation::QuotationMarks;

use crate::working_copy::{Awaited, Versioned};
use live_contract::FrameDTO;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub version: Version,
    pub name: String,
    pub updated_at: OffsetDateTime,
    pub trashed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectDetails {
    pub language: Option<String>,
    pub quotation: Option<QuotationMarks>,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescribedProject {
    pub version: Version,
    pub name: String,
    pub details: ProjectDetails,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashedProject {
    pub id: ProjectId,
    pub version: Version,
    pub name: String,
    pub until: OffsetDateTime,
    pub restored: bool,
}

/// What a change reported by the server does to one project in the list.
/// It is worked out from the project as currently held, if it is held at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Told {
    Changed(Project),
    Gone,
    Unchanged,
}

/// Returns what the project list must contain after a change to one project, as reported by the server.
/// A new reading of the list is used only once it contains this.
pub fn awaited_in_the_list(told: &FrameDTO) -> Option<Awaited<ProjectId>> {
    let id = ProjectId::from(told.of.clone()?);
    let event = serde_json::from_value::<ProjectEventDTO>(told.event.clone()).ok()?;

    Some(match event {
        ProjectEventDTO::Trashed { .. } | ProjectEventDTO::Deleted => Awaited::Gone(id),
        _ => Awaited::Held {
            id,
            at_least: told.version,
        },
    })
}

/// Returns what the trash must contain after a change to one project, as reported by the server.
/// A new reading of the trash is used only once it contains this.
/// Only changes that move the project into or out of the trash return something.
pub fn awaited_in_the_trash(told: &FrameDTO) -> Option<Awaited<ProjectId>> {
    let id = ProjectId::from(told.of.clone()?);

    match serde_json::from_value::<ProjectEventDTO>(told.event.clone()).ok()? {
        ProjectEventDTO::Trashed { .. } => Some(Awaited::Held { id, at_least: 0 }),
        ProjectEventDTO::Restored | ProjectEventDTO::Deleted => Some(Awaited::Gone(id)),
        _ => None,
    }
}

impl DescribedProject {
    pub fn trashed_by(&self, told: &FrameDTO) -> bool {
        told.version > self.version.0
            && matches!(
                serde_json::from_value::<ProjectEventDTO>(told.event.clone()),
                Ok(ProjectEventDTO::Trashed { .. })
            )
    }

    pub fn taken_in(mut self, told: FrameDTO) -> Option<Self> {
        if told.version <= self.version.0 {
            return Some(self);
        }
        if told.version != self.version.0 + 1 {
            return None;
        }
        match serde_json::from_value::<ProjectEventDTO>(told.event).ok()? {
            ProjectEventDTO::Renamed { name } => self.name = name,
            ProjectEventDTO::DetailsChanged {
                language,
                quotation,
                description,
            } => {
                self.details = ProjectDetails {
                    language,
                    quotation: quotation.as_deref().and_then(QuotationMarks::named),
                    description,
                }
            }
            ProjectEventDTO::QuotationChanged { quotation } => {
                self.details.quotation = QuotationMarks::named(&quotation)
            }
            ProjectEventDTO::Started { .. }
            | ProjectEventDTO::Trashed { .. }
            | ProjectEventDTO::Restored
            | ProjectEventDTO::Deleted => return None,
        }
        self.version = Version::from(told.version);

        Some(self)
    }
}

impl ProjectDetails {
    pub fn quotation_marks(&self) -> QuotationMarks {
        self.quotation
            .unwrap_or_else(|| QuotationMarks::of_language(self.language.as_deref()))
    }
}

pub fn told_about(held: Option<&Project>, told: FrameDTO) -> Option<Told> {
    let id = ProjectId::from(told.of?);
    let event = serde_json::from_value::<ProjectEventDTO>(told.event).ok()?;
    let version = Version::from(told.version);
    let at = || {
        told.at
            .as_deref()
            .and_then(|at| OffsetDateTime::parse(at, &Rfc3339).ok())
    };
    let Some(held) = held else {
        return match event {
            ProjectEventDTO::Started { name } if told.version == 1 => {
                Some(Told::Changed(Project {
                    id,
                    version,
                    name,
                    updated_at: at()?,
                    trashed: false,
                }))
            }
            ProjectEventDTO::Trashed { .. } | ProjectEventDTO::Deleted => Some(Told::Gone),
            _ => None,
        };
    };
    if told.version <= held.version.0 {
        return Some(Told::Unchanged);
    }
    if told.version != held.version.0 + 1 {
        return None;
    }
    let mut next = held.clone();
    match event {
        ProjectEventDTO::Renamed { name } => next.name = name,
        ProjectEventDTO::DetailsChanged { .. } | ProjectEventDTO::QuotationChanged { .. } => {}
        ProjectEventDTO::Trashed { .. } | ProjectEventDTO::Deleted => return Some(Told::Gone),
        ProjectEventDTO::Started { .. } | ProjectEventDTO::Restored => return None,
    }
    next.version = version;
    next.updated_at = at()?;

    Some(Told::Changed(next))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Membership {
    Begun(ProjectId),
    Ended(ProjectId),
}

const FIRST_WAIT: Duration = Duration::from_millis(100);
const LONGEST_WAIT: Duration = Duration::from_secs(1);
const GIVEN_UP_AFTER: Duration = Duration::from_secs(10);

pub fn membership_in(frame: FrameDTO) -> Option<Membership> {
    let project = ProjectId::from(frame.of?);

    match serde_json::from_value::<MembersEventDTO>(frame.event).ok()? {
        MembersEventDTO::Founded { .. } | MembersEventDTO::Joined { .. } => {
            Some(Membership::Begun(project))
        }
        MembersEventDTO::Left { .. } | MembersEventDTO::Removed { .. } => {
            Some(Membership::Ended(project))
        }
        MembersEventDTO::RoleChanged { .. }
        | MembersEventDTO::Disbanded
        | MembersEventDTO::Abandoned => None,
    }
}

pub fn next_wait(waited: Duration) -> Option<Duration> {
    (waited < GIVEN_UP_AFTER).then(|| waited.clamp(FIRST_WAIT, LONGEST_WAIT))
}

impl Version {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl Versioned for Project {
    fn version(&self) -> u64 {
        self.version.0
    }
}

impl Versioned for DescribedProject {
    fn version(&self) -> u64 {
        self.version.0
    }
}

impl Versioned for TrashedProject {
    fn version(&self) -> u64 {
        self.version.0
    }
}

impl From<String> for ProjectId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<u64> for Version {
    fn from(counted: u64) -> Self {
        Self(counted)
    }
}

impl From<Version> for u64 {
    fn from(version: Version) -> Self {
        version.0
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Display for ProjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod membership_tests {
        use serde_json::json;

        use super::*;

        fn told(of: Option<&str>, event: serde_json::Value) -> FrameDTO {
            FrameDTO {
                of: of.map(str::to_owned),
                version: 2,
                at: None,
                event,
            }
        }

        fn project_1() -> ProjectId {
            ProjectId::from("project_1".to_owned())
        }

        #[test]
        fn founding_or_joining_lets_one_into_the_project() {
            // expect: a founding and a joining each begin a membership
            for event in [
                json!({ "name": "FOUNDED", "owner": "account_1" }),
                json!({ "name": "JOINED", "account": "account_1", "role": "reader" }),
            ] {
                assert_eq!(
                    membership_in(told(Some("project_1"), event)),
                    Some(Membership::Begun(project_1()))
                );
            }
        }

        #[test]
        fn leaving_or_being_removed_lets_one_out_of_the_project() {
            // expect: a leaving and a removal each end a membership
            for event in [
                json!({ "name": "LEFT", "account": "account_1" }),
                json!({ "name": "REMOVED", "account": "account_1" }),
            ] {
                assert_eq!(
                    membership_in(told(Some("project_1"), event)),
                    Some(Membership::Ended(project_1()))
                );
            }
        }

        #[test]
        fn a_new_role_a_disbanding_or_a_frame_about_nothing_known_changes_no_list() {
            // expect: no membership begins or ends by any of them
            for frame in [
                told(
                    Some("project_1"),
                    json!({ "name": "ROLE_CHANGED", "account": "account_1", "to": "author" }),
                ),
                told(
                    Some("project_1"),
                    json!({ "name": "RENAMED", "project_name": "Weft" }),
                ),
                told(Some("project_1"), json!({ "name": "DISBANDED" })),
                told(
                    None,
                    json!({ "name": "JOINED", "account": "account_1", "role": "reader" }),
                ),
            ] {
                assert_eq!(membership_in(frame), None);
            }
        }

        #[test]
        fn a_project_one_was_let_into_is_asked_for_again_ever_more_slowly_for_ten_seconds() {
            // when: every wait is waited until there is none
            let mut waited = Duration::ZERO;
            let mut waits = Vec::new();
            while let Some(wait) = next_wait(waited) {
                waits.push(wait.as_millis());
                waited += wait;
            }

            // then: the waits double from a tenth of a second to one second
            assert_eq!(
                waits[..6],
                [100, 100, 200, 400, 800, 1_000],
                "quick at first, as the list is usually told within moments, then once a second"
            );
            // and: they end after about ten seconds
            assert_eq!(waited, Duration::from_millis(10_600));
            // and: stay at one second till then
            assert!(waits[5..].iter().all(|wait| *wait == 1_000));
        }
    }

    mod told_about_tests {
        use serde_json::json;
        use time::macros::datetime;

        use super::*;

        const AT: &str = "2026-10-21T12:00:00Z";

        fn a_project(id: &str, version: u64, name: &str) -> Project {
            Project {
                id: ProjectId::from(id.to_owned()),
                version: Version::from(version),
                name: name.to_owned(),
                updated_at: datetime!(2026-10-20 08:00 UTC),
                trashed: false,
            }
        }

        fn told(of: &str, version: u64, event: serde_json::Value) -> FrameDTO {
            FrameDTO {
                of: Some(of.to_owned()),
                version,
                at: Some(AT.to_owned()),
                event,
            }
        }

        fn changed_at_the_moment_told(id: &str, version: u64, name: &str) -> Option<Told> {
            Some(Told::Changed(Project {
                updated_at: datetime!(2026-10-21 12:00 UTC),
                ..a_project(id, version, name)
            }))
        }

        #[test]
        fn a_project_started_elsewhere_is_the_project_at_its_first_version() {
            // when: the start of a project not held is told
            let next = told_about(
                None,
                told(
                    "project_2",
                    1,
                    json!({ "name": "STARTED", "project_name": "Heddle" }),
                ),
            );

            // then: it is the project as started, updated when it was started
            assert_eq!(next, changed_at_the_moment_told("project_2", 1, "Heddle"));
        }

        #[test]
        fn a_rename_that_follows_on_renames_it_and_says_when() {
            // when: a rename of the next version is told
            let next = told_about(
                Some(&a_project("project_1", 1, "Loom")),
                told(
                    "project_1",
                    2,
                    json!({ "name": "RENAMED", "project_name": "Weft" }),
                ),
            );

            // then: the project is renamed at that version, updated when it was renamed
            assert_eq!(next, changed_at_the_moment_told("project_1", 2, "Weft"));
        }

        #[test]
        fn changed_details_and_marks_keep_the_name_and_say_when() {
            // when: a change of details or of marks is told
            for event in [
                json!({ "name": "DETAILS_CHANGED", "language": "de", "quotation": null, "description": "" }),
                json!({ "name": "QUOTATION_CHANGED", "quotation": "pointing-in" }),
            ] {
                let next = told_about(
                    Some(&a_project("project_1", 1, "Loom")),
                    told("project_1", 2, event),
                );

                // then: the project keeps its name at the new version, updated when it changed
                assert_eq!(next, changed_at_the_moment_told("project_1", 2, "Loom"));
            }
        }

        #[test]
        fn a_project_trashed_or_deleted_is_gone_whether_held_or_not() {
            // when: a trashing or a deletion is told of a project held and of one never held
            for event in [
                json!({ "name": "TRASHED", "until": "2026-12-03T10:00:00Z" }),
                json!({ "name": "DELETED" }),
            ] {
                let gone = told_about(
                    Some(&a_project("project_1", 2, "Loom")),
                    told("project_1", 3, event.clone()),
                );
                let never = told_about(None, told("project_9", 4, event.clone()));

                // then: both are gone from the list
                assert_eq!(gone, Some(Told::Gone), "{event}");
                assert_eq!(never, Some(Told::Gone), "{event}");
            }
        }

        #[test]
        fn what_is_held_already_changes_nothing() {
            // when: a rename at the version held is told
            let next = told_about(
                Some(&a_project("project_1", 3, "Weft")),
                told(
                    "project_1",
                    3,
                    json!({ "name": "RENAMED", "project_name": "Loom" }),
                ),
            );
            // and: the start of a project held
            let again = told_about(
                Some(&a_project("project_1", 1, "Loom")),
                told(
                    "project_1",
                    1,
                    json!({ "name": "STARTED", "project_name": "Loom" }),
                ),
            );

            // then: neither changes it
            assert_eq!(next, Some(Told::Unchanged));
            assert_eq!(again, Some(Told::Unchanged));
        }

        #[test]
        fn what_cannot_be_told_about_asks_for_the_list_again() {
            // given: changes that do not follow on from the project held, or from none
            let held = Some(a_project("project_1", 1, "Loom"));
            let renamed = json!({ "name": "RENAMED", "project_name": "Weft" });
            let started = json!({ "name": "STARTED", "project_name": "Weft" });
            let undated = FrameDTO {
                at: None,
                ..told("project_1", 2, renamed.clone())
            };
            let cases = [
                (
                    "a change further on than the next",
                    held.clone(),
                    told("project_1", 3, renamed.clone()),
                ),
                (
                    "a rename of a project not held",
                    None,
                    told("project_9", 2, renamed),
                ),
                (
                    "a start whose later changes were missed",
                    None,
                    told("project_9", 2, started.clone()),
                ),
                (
                    "a start of a held project further on",
                    held.clone(),
                    told("project_1", 2, started),
                ),
                (
                    "an event not known",
                    held.clone(),
                    told("project_1", 2, json!({ "name": "ARCHIVED" })),
                ),
                (
                    "a frame about no project",
                    held.clone(),
                    FrameDTO {
                        of: None,
                        ..told("project_1", 2, json!({ "name": "DELETED" }))
                    },
                ),
                ("a change that does not say when", held, undated),
            ];

            // then: none of them can be told about
            for (case, held, told) in cases {
                assert_eq!(told_about(held.as_ref(), told), None, "{case}");
            }
        }
    }

    mod described_taken_in_tests {
        use serde_json::json;

        use super::*;

        fn described(version: u64) -> DescribedProject {
            DescribedProject {
                version: Version::from(version),
                name: "The Loom".to_owned(),
                details: ProjectDetails {
                    language: Some("en".to_owned()),
                    quotation: None,
                    description: "A weaver's book".to_owned(),
                },
            }
        }

        fn told(version: u64, event: serde_json::Value) -> FrameDTO {
            FrameDTO {
                of: Some("project_1".to_owned()),
                version,
                at: None,
                event,
            }
        }

        #[test]
        fn a_rename_that_follows_on_renames_the_open_project() {
            // when: a rename of the next version is taken in
            let next = described(1)
                .taken_in(told(
                    2,
                    json!({ "name": "RENAMED", "project_name": "The Weft" }),
                ))
                .expect("a rename follows on");

            // then: the project is renamed at that version, its details kept
            assert_eq!(next.name, "The Weft");
            assert_eq!(next.version, Version::from(2));
            assert_eq!(next.details, described(1).details);
        }

        #[test]
        fn changed_details_replace_the_details_and_keep_the_name() {
            // when: a change of details is taken in
            let next = described(1)
                .taken_in(told(
                    2,
                    json!({ "name": "DETAILS_CHANGED", "language": "de", "quotation": "pointing-in", "description": "Ein Buch" }),
                ))
                .expect("a change of details follows on");

            // then: the details are the new ones
            assert_eq!(
                next.details,
                ProjectDetails {
                    language: Some("de".to_owned()),
                    quotation: QuotationMarks::named("pointing-in"),
                    description: "Ein Buch".to_owned(),
                }
            );
            // and: the name is kept at the new version
            assert_eq!(next.name, "The Loom");
            assert_eq!(next.version, Version::from(2));
        }

        #[test]
        fn changed_marks_change_only_the_marks() {
            // when: a change of marks is taken in
            let next = described(1)
                .taken_in(told(
                    2,
                    json!({ "name": "QUOTATION_CHANGED", "quotation": "pointing-in" }),
                ))
                .expect("a change of marks follows on");

            // then: the marks are the new ones, the rest kept at the new version
            assert_eq!(next.details.quotation, QuotationMarks::named("pointing-in"));
            assert_eq!(next.details.language.as_deref(), Some("en"));
            assert_eq!(next.version, Version::from(2));
        }

        #[test]
        fn a_change_already_held_changes_nothing() {
            // when: a rename at the version held is taken in
            let next = described(3).taken_in(told(
                3,
                json!({ "name": "RENAMED", "project_name": "The Weft" }),
            ));

            // then: the project is as it was
            assert_eq!(next, Some(described(3)));
        }

        #[test]
        fn what_cannot_be_taken_in_asks_for_the_project_again() {
            // given: changes that do not follow on from the open project
            let cases = [
                (
                    "a change further on than the next",
                    told(3, json!({ "name": "RENAMED", "project_name": "The Weft" })),
                ),
                ("the project deleted", told(2, json!({ "name": "DELETED" }))),
                (
                    "a start of a project already open",
                    told(2, json!({ "name": "STARTED", "project_name": "The Weft" })),
                ),
                ("an event not known", told(2, json!({ "name": "ARCHIVED" }))),
            ];

            // then: none of them can be taken in
            for (case, told) in cases {
                assert_eq!(described(1).taken_in(told), None, "{case}");
            }
        }

        #[test]
        fn a_trashing_told_after_what_is_held_says_the_open_project_went_to_the_trash() {
            // given: a trashing at any version
            let trashing = |version| {
                told(
                    version,
                    json!({ "name": "TRASHED", "until": "2026-12-02T10:00:00Z" }),
                )
            };

            // then: one after the version held trashes the open project, however far after
            assert!(described(1).trashed_by(&trashing(2)));
            assert!(
                described(1).trashed_by(&trashing(4)),
                "whatever came between, the project is in the trash now"
            );
            // and: one already held does not
            assert!(
                !described(2).trashed_by(&trashing(2)),
                "a trashing already held is old news"
            );
            // and: neither does a deletion or a restoring
            assert!(!described(1).trashed_by(&told(2, json!({ "name": "DELETED" }))));
            assert!(!described(1).trashed_by(&told(2, json!({ "name": "RESTORED" }))));
        }
    }

    mod awaited_tests {
        use serde_json::json;
        use time::macros::datetime;

        use super::*;

        fn told(version: u64, event: serde_json::Value) -> FrameDTO {
            FrameDTO {
                of: Some("project_1".to_owned()),
                version,
                at: None,
                event,
            }
        }

        fn project_1() -> ProjectId {
            ProjectId::from("project_1".to_owned())
        }

        fn listed(version: u64) -> Vec<(ProjectId, Project)> {
            vec![(
                project_1(),
                Project {
                    id: project_1(),
                    version: Version::from(version),
                    name: "The Loom".to_owned(),
                    updated_at: datetime!(2026-11-03 10:00 UTC),
                    trashed: false,
                },
            )]
        }

        fn none_listed() -> Vec<(ProjectId, Project)> {
            Vec::new()
        }

        fn in_the_trash() -> Vec<(ProjectId, TrashedProject)> {
            vec![(
                project_1(),
                TrashedProject {
                    id: project_1(),
                    version: Version::from(2),
                    name: "The Loom".to_owned(),
                    until: datetime!(2026-12-03 10:00 UTC),
                    restored: false,
                },
            )]
        }

        fn none_in_the_trash() -> Vec<(ProjectId, TrashedProject)> {
            Vec::new()
        }

        #[test]
        fn a_change_told_awaits_the_project_listed_at_least_at_its_version() {
            // when: a change is told
            let awaited = awaited_in_the_list(&told(
                3,
                json!({ "name": "RENAMED", "project_name": "The Weft" }),
            ))
            .expect("a rename is awaited");

            // then: a list without the project or with it older does not hold it
            assert!(
                !awaited.holds(&none_listed()),
                "a list without it is behind"
            );
            assert!(
                !awaited.holds(&listed(2)),
                "a list holding it older is behind"
            );
            // and: one with it at that version or later does
            assert!(awaited.holds(&listed(3)));
            assert!(awaited.holds(&listed(4)));
        }

        #[test]
        fn a_project_trashed_or_deleted_is_awaited_gone_from_the_list() {
            // when: a trashing or a deletion is told
            for event in [
                json!({ "name": "TRASHED", "until": "2026-12-03T10:00:00Z" }),
                json!({ "name": "DELETED" }),
            ] {
                let awaited = awaited_in_the_list(&told(3, event.clone())).expect("awaited");

                // then: only a list without the project holds it
                assert!(!awaited.holds(&listed(2)), "{event}");
                assert!(awaited.holds(&none_listed()), "{event}");
            }
        }

        #[test]
        fn a_frame_that_names_nothing_awaits_nothing() {
            // given: a frame about no project
            let nameless = FrameDTO {
                of: None,
                version: 3,
                at: None,
                event: json!({ "name": "RENAMED", "project_name": "The Weft" }),
            };

            // then: neither it nor an unknown event awaits anything
            assert_eq!(awaited_in_the_list(&nameless), None);
            assert_eq!(
                awaited_in_the_list(&told(3, json!({ "name": "ARCHIVED" }))),
                None
            );
        }

        #[test]
        fn a_project_trashed_is_awaited_in_the_trash_and_one_restored_out_of_it() {
            // when: a trashing and a restoring are told
            let trashed = awaited_in_the_trash(&told(
                2,
                json!({ "name": "TRASHED", "until": "2026-12-03T10:00:00Z" }),
            ))
            .expect("a trashing is awaited in the trash");
            let restored = awaited_in_the_trash(&told(3, json!({ "name": "RESTORED" })))
                .expect("a restoring is awaited out of it");

            // then: only a trash with the project holds the trashing
            assert!(!trashed.holds(&none_in_the_trash()));
            assert!(trashed.holds(&in_the_trash()));
            // and: only a trash without it holds the restoring
            assert!(!restored.holds(&in_the_trash()));
            assert!(restored.holds(&none_in_the_trash()));
        }

        #[test]
        fn a_project_purged_is_awaited_out_of_the_trash() {
            // when: a deletion is told
            let purged = awaited_in_the_trash(&told(4, json!({ "name": "DELETED" })))
                .expect("a project purged leaves the trash, so the trash is asked again");

            // then: only a trash without the project holds it
            assert!(!purged.holds(&in_the_trash()));
            assert!(purged.holds(&none_in_the_trash()));
        }

        #[test]
        fn nothing_but_trashing_restoring_and_purging_is_awaited_in_the_trash() {
            // expect: a rename or an unknown event awaits nothing in the trash
            for event in [
                json!({ "name": "RENAMED", "project_name": "The Weft" }),
                json!({ "name": "ARCHIVED" }),
            ] {
                assert_eq!(
                    awaited_in_the_trash(&told(2, event.clone())),
                    None,
                    "{event}"
                );
            }
        }
    }
}
