use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use members_contract::MembersEventDTO;
use projects_contract::ProjectEventDTO;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use quotation::QuotationMarks;

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

impl DescribedProject {
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
            ProjectEventDTO::Started { .. } | ProjectEventDTO::Deleted => return None,
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

pub fn taken_in(mut held: Vec<Project>, told: FrameDTO) -> Option<Vec<Project>> {
    let id = ProjectId::from(told.of?);
    let event = serde_json::from_value::<ProjectEventDTO>(told.event).ok()?;
    let version = Version::from(told.version);
    let at = || {
        told.at
            .as_deref()
            .and_then(|at| OffsetDateTime::parse(at, &Rfc3339).ok())
    };
    let Some(index) = held.iter().position(|project| project.id == id) else {
        match event {
            ProjectEventDTO::Started { name } if told.version == 1 => held.push(Project {
                id,
                version,
                name,
                updated_at: at()?,
            }),
            ProjectEventDTO::Deleted => {}
            _ => return None,
        }
        held.sort_by(|one, other| other.id.cmp(&one.id));

        return Some(held);
    };
    let known = &mut held[index];
    if told.version <= known.version.0 {
        return Some(held);
    }
    if told.version != known.version.0 + 1 {
        return None;
    }
    match event {
        ProjectEventDTO::Renamed { name } => known.name = name,
        ProjectEventDTO::DetailsChanged { .. } | ProjectEventDTO::QuotationChanged { .. } => {}
        ProjectEventDTO::Deleted => {
            held.remove(index);
            return Some(held);
        }
        ProjectEventDTO::Started { .. } => return None,
    }
    known.version = version;
    known.updated_at = at()?;

    Some(held)
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
        MembersEventDTO::RoleChanged { .. } => None,
    }
}

pub fn next_wait(waited: Duration) -> Option<Duration> {
    (waited < GIVEN_UP_AFTER).then(|| waited.clamp(FIRST_WAIT, LONGEST_WAIT))
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
        fn a_new_role_or_a_frame_about_nothing_known_changes_no_list() {
            for frame in [
                told(
                    Some("project_1"),
                    json!({ "name": "ROLE_CHANGED", "account": "account_1", "to": "author" }),
                ),
                told(
                    Some("project_1"),
                    json!({ "name": "RENAMED", "project_name": "Weft" }),
                ),
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
            let mut waited = Duration::ZERO;
            let mut waits = Vec::new();
            while let Some(wait) = next_wait(waited) {
                waits.push(wait.as_millis());
                waited += wait;
            }

            assert_eq!(
                waits[..6],
                [100, 100, 200, 400, 800, 1_000],
                "quick at first, as the list is usually told within moments, then once a second"
            );
            assert_eq!(waited, Duration::from_millis(10_600));
            assert!(waits[5..].iter().all(|wait| *wait == 1_000));
        }
    }

    mod taken_in_tests {
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

        fn named(held: &[Project]) -> Vec<(&str, u64)> {
            held.iter()
                .map(|project| (project.name.as_str(), project.version.0))
                .collect()
        }

        #[test]
        fn a_project_started_elsewhere_joins_the_list_in_its_place() {
            let next = taken_in(
                vec![
                    a_project("project_3", 1, "Weft"),
                    a_project("project_1", 1, "Loom"),
                ],
                told(
                    "project_2",
                    1,
                    json!({ "name": "STARTED", "project_name": "Heddle" }),
                ),
            )
            .expect("a started project follows on from nothing");

            assert_eq!(named(&next), vec![("Weft", 1), ("Heddle", 1), ("Loom", 1)]);
            assert_eq!(next[1].updated_at, datetime!(2026-10-21 12:00 UTC));
        }

        #[test]
        fn a_rename_that_follows_on_renames_it_and_says_when() {
            let next = taken_in(
                vec![a_project("project_1", 1, "Loom")],
                told(
                    "project_1",
                    2,
                    json!({ "name": "RENAMED", "project_name": "Weft" }),
                ),
            )
            .expect("a rename follows on");

            assert_eq!(named(&next), vec![("Weft", 2)]);
            assert_eq!(next[0].updated_at, datetime!(2026-10-21 12:00 UTC));
        }

        #[test]
        fn changed_details_and_marks_keep_the_name_and_say_when() {
            for event in [
                json!({ "name": "DETAILS_CHANGED", "language": "de", "quotation": null, "description": "" }),
                json!({ "name": "QUOTATION_CHANGED", "quotation": "pointing-in" }),
            ] {
                let next = taken_in(
                    vec![a_project("project_1", 1, "Loom")],
                    told("project_1", 2, event),
                )
                .expect("a change of details follows on");

                assert_eq!(named(&next), vec![("Loom", 2)]);
                assert_eq!(next[0].updated_at, datetime!(2026-10-21 12:00 UTC));
            }
        }

        #[test]
        fn a_deleted_project_leaves_and_one_never_held_is_gone_already() {
            let held = vec![a_project("project_1", 2, "Loom")];

            let gone = taken_in(
                held.clone(),
                told("project_1", 3, json!({ "name": "DELETED" })),
            );
            let never = taken_in(held, told("project_9", 4, json!({ "name": "DELETED" })));

            assert_eq!(gone.as_deref().map(named), Some(vec![]));
            assert_eq!(never.as_deref().map(named), Some(vec![("Loom", 2)]));
        }

        #[test]
        fn what_is_held_already_changes_nothing() {
            let next = taken_in(
                vec![a_project("project_1", 3, "Weft")],
                told(
                    "project_1",
                    3,
                    json!({ "name": "RENAMED", "project_name": "Loom" }),
                ),
            );
            let again = taken_in(
                vec![a_project("project_1", 1, "Loom")],
                told(
                    "project_1",
                    1,
                    json!({ "name": "STARTED", "project_name": "Loom" }),
                ),
            );

            assert_eq!(next.as_deref().map(named), Some(vec![("Weft", 3)]));
            assert_eq!(again.as_deref().map(named), Some(vec![("Loom", 1)]));
        }

        #[test]
        fn what_cannot_be_taken_in_asks_for_the_list_again() {
            let undated = FrameDTO {
                at: None,
                ..told(
                    "project_1",
                    2,
                    json!({ "name": "RENAMED", "project_name": "Weft" }),
                )
            };
            let cases = [
                (
                    "a change further on than the next",
                    told(
                        "project_1",
                        3,
                        json!({ "name": "RENAMED", "project_name": "Weft" }),
                    ),
                ),
                (
                    "a rename of a project not held",
                    told(
                        "project_9",
                        2,
                        json!({ "name": "RENAMED", "project_name": "Weft" }),
                    ),
                ),
                (
                    "a start whose later changes were missed",
                    told(
                        "project_9",
                        2,
                        json!({ "name": "STARTED", "project_name": "Weft" }),
                    ),
                ),
                (
                    "a start of a held project further on",
                    told(
                        "project_1",
                        2,
                        json!({ "name": "STARTED", "project_name": "Weft" }),
                    ),
                ),
                (
                    "an event not known",
                    told("project_1", 2, json!({ "name": "ARCHIVED" })),
                ),
                (
                    "a frame about no project",
                    FrameDTO {
                        of: None,
                        ..told("project_1", 2, json!({ "name": "DELETED" }))
                    },
                ),
                ("a change that does not say when", undated),
            ];

            for (case, told) in cases {
                assert_eq!(
                    taken_in(vec![a_project("project_1", 1, "Loom")], told),
                    None,
                    "{case}"
                );
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
            let next = described(1)
                .taken_in(told(
                    2,
                    json!({ "name": "RENAMED", "project_name": "The Weft" }),
                ))
                .expect("a rename follows on");

            assert_eq!(next.name, "The Weft");
            assert_eq!(next.version, Version::from(2));
            assert_eq!(next.details, described(1).details);
        }

        #[test]
        fn changed_details_replace_the_details_and_keep_the_name() {
            let next = described(1)
                .taken_in(told(
                    2,
                    json!({ "name": "DETAILS_CHANGED", "language": "de", "quotation": "pointing-in", "description": "Ein Buch" }),
                ))
                .expect("a change of details follows on");

            assert_eq!(
                next.details,
                ProjectDetails {
                    language: Some("de".to_owned()),
                    quotation: QuotationMarks::named("pointing-in"),
                    description: "Ein Buch".to_owned(),
                }
            );
            assert_eq!(next.name, "The Loom");
            assert_eq!(next.version, Version::from(2));
        }

        #[test]
        fn changed_marks_change_only_the_marks() {
            let next = described(1)
                .taken_in(told(
                    2,
                    json!({ "name": "QUOTATION_CHANGED", "quotation": "pointing-in" }),
                ))
                .expect("a change of marks follows on");

            assert_eq!(next.details.quotation, QuotationMarks::named("pointing-in"));
            assert_eq!(next.details.language.as_deref(), Some("en"));
            assert_eq!(next.version, Version::from(2));
        }

        #[test]
        fn a_change_already_held_changes_nothing() {
            let next = described(3).taken_in(told(
                3,
                json!({ "name": "RENAMED", "project_name": "The Weft" }),
            ));

            assert_eq!(next, Some(described(3)));
        }

        #[test]
        fn what_cannot_be_taken_in_asks_for_the_project_again() {
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

            for (case, told) in cases {
                assert_eq!(described(1).taken_in(told), None, "{case}");
            }
        }
    }
}
