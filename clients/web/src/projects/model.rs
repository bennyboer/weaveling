use std::fmt::{self, Display, Formatter};

use projects_contract::ProjectEventDTO;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use quotation::QuotationMarks;

use crate::live::Told;

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

impl ProjectDetails {
    pub fn quotation_marks(&self) -> QuotationMarks {
        self.quotation
            .unwrap_or_else(|| QuotationMarks::of_language(self.language.as_deref()))
    }
}

pub fn taken_in(mut held: Vec<Project>, told: Told) -> Option<Vec<Project>> {
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

        fn told(of: &str, version: u64, event: serde_json::Value) -> Told {
            Told {
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
                json!({ "name": "QUOTATION_CHANGED", "quotation": "german" }),
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
            let undated = Told {
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
                    Told {
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
}
