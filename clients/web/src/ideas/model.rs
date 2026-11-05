use std::fmt::{self, Display, Formatter};

use ideas_contract::IdeaEventDTO;

use crate::projects::model::ProjectId;
use crate::speech;
use live_contract::FrameDTO;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Awaited {
    Held { id: IdeaId, at_least: u64 },
    Gone(IdeaId),
}

impl Awaited {
    pub fn told(told: &FrameDTO) -> Option<Self> {
        let id = IdeaId::from(told.of.clone()?);

        Some(
            match serde_json::from_value::<IdeaEventDTO>(told.event.clone()).ok()? {
                IdeaEventDTO::Discarded => Self::Gone(id),
                _ => Self::Held {
                    id,
                    at_least: told.version,
                },
            },
        )
    }

    pub fn holds(&self, held: &[Idea]) -> bool {
        match self {
            Self::Held { id, at_least } => held
                .iter()
                .any(|idea| idea.id == *id && idea.version >= *at_least),
            Self::Gone(id) => held.iter().all(|idea| idea.id != *id),
        }
    }
}

/// What a change told of one idea makes of it, given the idea as held, if held at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Told {
    Changed(Idea),
    Discarded,
    Unchanged,
}

pub fn told_about(held: Option<&Idea>, told: FrameDTO) -> Option<Told> {
    let id = IdeaId::from(told.of?);
    let event = serde_json::from_value::<IdeaEventDTO>(told.event).ok()?;
    let Some(held) = held else {
        return match event {
            IdeaEventDTO::Captured { project, title } if told.version == 1 => {
                Some(Told::Changed(Idea {
                    id,
                    version: told.version,
                    project: ProjectId::from(project),
                    title,
                }))
            }
            IdeaEventDTO::Discarded => Some(Told::Discarded),
            _ => None,
        };
    };
    if told.version <= held.version {
        return Some(Told::Unchanged);
    }
    if told.version != held.version + 1 {
        return None;
    }
    match event {
        IdeaEventDTO::Retitled { title } => Some(Told::Changed(Idea {
            title,
            version: told.version,
            ..held.clone()
        })),
        IdeaEventDTO::Discarded => Some(Told::Discarded),
        IdeaEventDTO::Captured { .. } => None,
    }
}

pub fn taken_in(mut held: Vec<Idea>, told: FrameDTO) -> Option<Vec<Idea>> {
    let id = IdeaId::from(told.of.clone()?);
    let at = held.iter().position(|idea| idea.id == id);
    match (told_about(at.map(|at| &held[at]), told)?, at) {
        (Told::Changed(idea), Some(at)) => held[at] = idea,
        (Told::Changed(idea), None) => held.push(idea),
        (Told::Discarded, Some(at)) => _ = held.remove(at),
        (Told::Discarded | Told::Unchanged, _) => {}
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
        // expect: a titled idea shows its title
        assert_eq!(an_idea("The loom").shown_as(), "The loom");
    }

    #[test]
    fn an_idea_without_a_title_is_shown_as_untitled() {
        // expect: an untitled idea shows as untitled
        assert_eq!(an_idea("").shown_as(), "Untitled");
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

        fn told(of: &str, version: u64, event: serde_json::Value) -> FrameDTO {
            FrameDTO {
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
            // when: the capture of an idea not held is taken in
            let next = taken_in(
                held(1, "The loom"),
                told(
                    "idea_2",
                    1,
                    json!({ "name": "CAPTURED", "project": "project_1", "title": "The shuttle" }),
                ),
            )
            .expect("a captured idea follows on from nothing");

            // then: it is held after the others with its title and project
            assert_eq!(titles(&next), vec![("The loom", 1), ("The shuttle", 1)]);
            assert_eq!(next[1].id, IdeaId::from("idea_2".to_owned()));
            assert_eq!(next[1].project, ProjectId::from("project_1".to_owned()));
        }

        #[test]
        fn an_idea_already_held_is_not_captured_twice() {
            // when: the capture of an idea held is taken in
            let next = taken_in(
                held(1, "The loom"),
                told(
                    "idea_1",
                    1,
                    json!({ "name": "CAPTURED", "project": "project_1", "title": "The loom" }),
                ),
            );

            // then: it is held once
            assert_eq!(next.as_deref().map(titles), Some(vec![("The loom", 1)]));
        }

        #[test]
        fn a_retitle_that_follows_on_is_taken_in() {
            // when: a retitle of the next version is taken in
            let next = taken_in(
                held(1, "The loom"),
                told(
                    "idea_1",
                    2,
                    json!({ "name": "RETITLED", "title": "The weft" }),
                ),
            );

            // then: the idea is held under its new title at that version
            assert_eq!(next.as_deref().map(titles), Some(vec![("The weft", 2)]));
        }

        #[test]
        fn a_change_already_held_changes_nothing() {
            // when: a change at the version held is taken in
            let next = taken_in(
                held(3, "The weft"),
                told(
                    "idea_1",
                    3,
                    json!({ "name": "RETITLED", "title": "The loom" }),
                ),
            );

            // then: the idea is held as it was
            assert_eq!(next.as_deref().map(titles), Some(vec![("The weft", 3)]));
        }

        #[test]
        fn a_discarded_idea_leaves() {
            // when: the discarding of a held idea is taken in
            let next = taken_in(
                held(2, "The loom"),
                told("idea_1", 3, json!({ "name": "DISCARDED" })),
            );

            // then: no idea is held
            assert_eq!(next.as_deref().map(titles), Some(vec![]));
        }

        #[test]
        fn an_idea_discarded_that_was_never_held_is_gone_already() {
            // when: the discarding of an idea not held is taken in
            let next = taken_in(
                held(1, "The loom"),
                told("idea_9", 4, json!({ "name": "DISCARDED" })),
            );

            // then: the held ones are held as they were
            assert_eq!(next.as_deref().map(titles), Some(vec![("The loom", 1)]));
        }

        #[test]
        fn what_cannot_be_taken_in_asks_for_the_ideas_again() {
            // given: changes that do not follow on from the ideas held
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
                    FrameDTO {
                        of: None,
                        version: 2,
                        at: None,
                        event: json!({ "name": "DISCARDED" }),
                    },
                ),
            ];

            // then: none of them can be taken in
            for (case, told) in cases {
                assert_eq!(taken_in(held(1, "The loom"), told), None, "{case}");
            }
        }
    }

    mod told_about_tests {
        use serde_json::json;

        use super::*;

        fn told(version: u64, event: serde_json::Value) -> FrameDTO {
            FrameDTO {
                of: Some("idea_1".to_owned()),
                version,
                at: None,
                event,
            }
        }

        fn held(version: u64) -> Idea {
            Idea {
                version,
                ..an_idea("The loom")
            }
        }

        #[test]
        fn a_retitle_that_follows_on_changes_the_idea_held() {
            // when: a retitle of the next version is told of a held idea
            let next = told_about(
                Some(&held(1)),
                told(2, json!({ "name": "RETITLED", "title": "The weft" })),
            );

            // then: the idea is changed to the new title at that version
            assert_eq!(
                next,
                Some(Told::Changed(Idea {
                    title: "The weft".to_owned(),
                    ..held(2)
                }))
            );
        }

        #[test]
        fn a_capture_of_an_idea_not_held_is_the_idea_at_its_first_version() {
            // when: the capture of an idea not held is told
            let next = told_about(
                None,
                told(
                    1,
                    json!({ "name": "CAPTURED", "project": "project_1", "title": "The loom" }),
                ),
            );

            // then: it is the idea as captured
            assert_eq!(next, Some(Told::Changed(held(1))));
        }

        #[test]
        fn a_discard_of_an_idea_held_or_not_lets_it_go() {
            // when: a discard is told of a held idea and of one not held
            let of_held = told_about(Some(&held(1)), told(2, json!({ "name": "DISCARDED" })));
            let of_none = told_about(None, told(2, json!({ "name": "DISCARDED" })));

            // then: both are discarded
            assert_eq!(of_held, Some(Told::Discarded));
            assert_eq!(of_none, Some(Told::Discarded));
        }

        #[test]
        fn a_change_already_held_leaves_the_idea_unchanged() {
            // when: a change at the version held is told
            let next = told_about(
                Some(&held(3)),
                told(3, json!({ "name": "RETITLED", "title": "The weft" })),
            );

            // then: the idea is unchanged
            assert_eq!(next, Some(Told::Unchanged));
        }

        #[test]
        fn a_change_further_on_than_the_next_cannot_be_told_about() {
            // when: a change two versions on is told
            let next = told_about(
                Some(&held(1)),
                told(3, json!({ "name": "RETITLED", "title": "The weft" })),
            );

            // then: it cannot be made out from the idea held
            assert_eq!(next, None);
        }
    }

    mod awaited_tests {
        use serde_json::json;

        use super::*;

        fn told(version: u64, event: serde_json::Value) -> FrameDTO {
            FrameDTO {
                of: Some("idea_1".to_owned()),
                version,
                at: None,
                event,
            }
        }

        fn held(version: u64) -> Vec<Idea> {
            vec![Idea {
                version,
                ..an_idea("The loom")
            }]
        }

        #[test]
        fn a_change_told_awaits_the_idea_held_at_least_at_its_version() {
            // when: a change is told
            let awaited =
                Awaited::told(&told(2, json!({ "name": "RETITLED", "title": "The weft" })))
                    .expect("a retitle is awaited");

            // then: neither no idea nor the idea before it holds it
            assert!(!awaited.holds(&[]));
            assert!(
                !awaited.holds(&held(1)),
                "an answer read before the change is behind"
            );
            // and: the idea at its version does
            assert!(awaited.holds(&held(2)));
        }

        #[test]
        fn a_discard_told_awaits_the_idea_gone() {
            // when: a discard is told
            let awaited = Awaited::told(&told(2, json!({ "name": "DISCARDED" })))
                .expect("a discard is awaited");

            // then: the idea still held does not hold it
            assert!(!awaited.holds(&held(1)));
            // and: the idea gone does
            assert!(awaited.holds(&[]));
        }

        #[test]
        fn a_frame_that_names_nothing_awaits_nothing() {
            // given: a frame about no idea
            let nameless = FrameDTO {
                of: None,
                version: 2,
                at: None,
                event: json!({ "name": "DISCARDED" }),
            };

            // then: neither it nor an unknown event awaits anything
            assert_eq!(Awaited::told(&nameless), None);
            assert_eq!(Awaited::told(&told(2, json!({ "name": "ARCHIVED" }))), None);
        }
    }
}
