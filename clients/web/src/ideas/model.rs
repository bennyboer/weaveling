use std::fmt::{self, Display, Formatter};

use ideas_contract::IdeaEventDTO;

use crate::live::Told;
use crate::projects::model::ProjectId;
use crate::speech;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdeaId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Idea {
    pub id: IdeaId,
    pub version: u64,
    pub project: ProjectId,
    pub title: String,
}

impl From<String> for IdeaId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for IdeaId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Idea {
    pub fn shown_as(&self) -> String {
        if self.title.is_empty() {
            speech::say("untitled")
        } else {
            self.title.clone()
        }
    }
}

pub fn hold(held: &mut Vec<Idea>, newer: Idea) {
    match held.iter_mut().find(|known| known.id == newer.id) {
        Some(known) if known.version < newer.version => *known = newer,
        Some(_) => {}
        None => held.push(newer),
    }
}

pub fn taken_in(mut held: Vec<Idea>, told: Told) -> Option<Vec<Idea>> {
    let id = IdeaId::from(told.of?);
    let event = serde_json::from_value::<IdeaEventDTO>(told.event).ok()?;
    let Some(at) = held.iter().position(|idea| idea.id == id) else {
        match event {
            IdeaEventDTO::Captured { project, title } if told.version == 1 => held.push(Idea {
                id,
                version: told.version,
                project: ProjectId::from(project),
                title,
            }),
            IdeaEventDTO::Discarded => {}
            _ => return None,
        }

        return Some(held);
    };
    if told.version <= held[at].version {
        return Some(held);
    }
    if told.version != held[at].version + 1 {
        return None;
    }
    match event {
        IdeaEventDTO::Retitled { title } => {
            held[at].title = title;
            held[at].version = told.version;
        }
        IdeaEventDTO::Discarded => {
            held.remove(at);
        }
        IdeaEventDTO::Captured { .. } => return None,
    }

    Some(held)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn an_idea(title: &str) -> Idea {
        Idea {
            id: IdeaId::from("idea_1".to_owned()),
            version: 1,
            project: ProjectId::from("project_1".to_owned()),
            title: title.to_owned(),
        }
    }

    #[test]
    fn an_idea_is_shown_by_its_title() {
        assert_eq!(an_idea("The loom").shown_as(), "The loom");
    }

    #[test]
    fn an_idea_without_a_title_is_shown_as_untitled() {
        assert_eq!(an_idea("").shown_as(), "Untitled");
    }

    mod hold_tests {
        use super::*;

        fn versioned(title: &str, version: u64) -> Idea {
            Idea {
                version,
                ..an_idea(title)
            }
        }

        #[test]
        fn an_idea_not_held_is_added() {
            let mut held = Vec::new();

            hold(&mut held, versioned("The loom", 1));

            assert_eq!(held, vec![versioned("The loom", 1)]);
        }

        #[test]
        fn an_idea_already_held_is_not_held_twice() {
            let mut held = vec![versioned("The loom", 1)];

            hold(&mut held, versioned("The loom", 1));

            assert_eq!(held, vec![versioned("The loom", 1)]);
        }

        #[test]
        fn a_newer_version_replaces_the_one_held() {
            let mut held = vec![versioned("The loom", 1)];

            hold(&mut held, versioned("The weft", 2));

            assert_eq!(held, vec![versioned("The weft", 2)]);
        }

        #[test]
        fn an_older_version_leaves_the_newer_one_held() {
            let mut held = vec![versioned("The heddle", 3)];

            hold(&mut held, versioned("The weft", 2));

            assert_eq!(held, vec![versioned("The heddle", 3)]);
        }
    }

    mod taken_in_tests {
        use serde_json::json;

        use super::*;

        fn held(version: u64, title: &str) -> Vec<Idea> {
            vec![Idea {
                version,
                ..an_idea(title)
            }]
        }

        fn told(of: &str, version: u64, event: serde_json::Value) -> Told {
            Told {
                of: Some(of.to_owned()),
                version,
                at: None,
                event,
            }
        }

        fn titles(ideas: &[Idea]) -> Vec<(&str, u64)> {
            ideas
                .iter()
                .map(|idea| (idea.title.as_str(), idea.version))
                .collect()
        }

        #[test]
        fn an_idea_captured_elsewhere_joins_the_held_ones() {
            let next = taken_in(
                held(1, "The loom"),
                told(
                    "idea_2",
                    1,
                    json!({ "name": "CAPTURED", "project": "project_1", "title": "The shuttle" }),
                ),
            )
            .expect("a captured idea follows on from nothing");

            assert_eq!(titles(&next), vec![("The loom", 1), ("The shuttle", 1)]);
            assert_eq!(next[1].id, IdeaId::from("idea_2".to_owned()));
            assert_eq!(next[1].project, ProjectId::from("project_1".to_owned()));
        }

        #[test]
        fn an_idea_already_held_is_not_captured_twice() {
            let next = taken_in(
                held(1, "The loom"),
                told(
                    "idea_1",
                    1,
                    json!({ "name": "CAPTURED", "project": "project_1", "title": "The loom" }),
                ),
            );

            assert_eq!(next.as_deref().map(titles), Some(vec![("The loom", 1)]));
        }

        #[test]
        fn a_retitle_that_follows_on_is_taken_in() {
            let next = taken_in(
                held(1, "The loom"),
                told(
                    "idea_1",
                    2,
                    json!({ "name": "RETITLED", "title": "The weft" }),
                ),
            );

            assert_eq!(next.as_deref().map(titles), Some(vec![("The weft", 2)]));
        }

        #[test]
        fn a_change_already_held_changes_nothing() {
            let next = taken_in(
                held(3, "The weft"),
                told(
                    "idea_1",
                    3,
                    json!({ "name": "RETITLED", "title": "The loom" }),
                ),
            );

            assert_eq!(next.as_deref().map(titles), Some(vec![("The weft", 3)]));
        }

        #[test]
        fn a_discarded_idea_leaves() {
            let next = taken_in(
                held(2, "The loom"),
                told("idea_1", 3, json!({ "name": "DISCARDED" })),
            );

            assert_eq!(next.as_deref().map(titles), Some(vec![]));
        }

        #[test]
        fn an_idea_discarded_that_was_never_held_is_gone_already() {
            let next = taken_in(
                held(1, "The loom"),
                told("idea_9", 4, json!({ "name": "DISCARDED" })),
            );

            assert_eq!(next.as_deref().map(titles), Some(vec![("The loom", 1)]));
        }

        #[test]
        fn what_cannot_be_taken_in_asks_for_the_ideas_again() {
            let cases = [
                (
                    "a change further on than the next",
                    told(
                        "idea_1",
                        3,
                        json!({ "name": "RETITLED", "title": "The weft" }),
                    ),
                ),
                (
                    "a retitle of an idea not held",
                    told(
                        "idea_9",
                        2,
                        json!({ "name": "RETITLED", "title": "The weft" }),
                    ),
                ),
                (
                    "a capture whose earlier changes were missed",
                    told(
                        "idea_9",
                        2,
                        json!({ "name": "CAPTURED", "project": "project_1", "title": "The weft" }),
                    ),
                ),
                (
                    "a capture of a held idea further on",
                    told(
                        "idea_1",
                        2,
                        json!({ "name": "CAPTURED", "project": "project_1", "title": "The weft" }),
                    ),
                ),
                (
                    "an event not known",
                    told("idea_1", 2, json!({ "name": "BRAIDED" })),
                ),
                (
                    "a frame about no idea",
                    Told {
                        of: None,
                        version: 2,
                        at: None,
                        event: json!({ "name": "DISCARDED" }),
                    },
                ),
            ];

            for (case, told) in cases {
                assert_eq!(taken_in(held(1, "The loom"), told), None, "{case}");
            }
        }
    }
}
