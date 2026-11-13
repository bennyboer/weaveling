use std::fmt::{self, Display, Formatter};

use ideas_contract::IdeaEventDTO;

use crate::projects::model::ProjectId;
use crate::speech;
use crate::working_copy::Awaited;
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

/// Returns what the ideas must contain after a change to one idea, as reported by the server.
/// A new reading of the ideas is used only once it contains this.
pub fn awaited(told: &FrameDTO) -> Option<Awaited<IdeaId>> {
    let id = IdeaId::from(told.of.clone()?);

    Some(
        match serde_json::from_value::<IdeaEventDTO>(told.event.clone()).ok()? {
            IdeaEventDTO::Discarded => Awaited::Gone(id),
            _ => Awaited::Held {
                id,
                at_least: told.version,
            },
        },
    )
}

/// What a change reported by the server does to one idea.
/// It is worked out from the idea as currently held, if it is held at all.
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
        fn an_idea_held_is_not_captured_twice() {
            // when: the capture of an idea held is told
            let next = told_about(
                Some(&held(1)),
                told(
                    1,
                    json!({ "name": "CAPTURED", "project": "project_1", "title": "The loom" }),
                ),
            );

            // then: the idea is unchanged
            assert_eq!(next, Some(Told::Unchanged));
        }

        #[test]
        fn what_does_not_follow_on_from_what_is_held_cannot_be_told_about() {
            // given: changes that do not follow on from the idea held, or from none
            let further = json!({ "name": "RETITLED", "title": "The weft" });
            let captured =
                json!({ "name": "CAPTURED", "project": "project_1", "title": "The weft" });
            let cases = [
                (
                    "a change further on than the next",
                    Some(held(1)),
                    told(3, further.clone()),
                ),
                ("a retitle of an idea not held", None, told(2, further)),
                (
                    "a capture whose earlier changes were missed",
                    None,
                    told(2, captured.clone()),
                ),
                (
                    "a capture of a held idea further on",
                    Some(held(1)),
                    told(2, captured),
                ),
                (
                    "an event not known",
                    Some(held(1)),
                    told(2, json!({ "name": "BRAIDED" })),
                ),
                (
                    "a frame about no idea",
                    Some(held(1)),
                    FrameDTO {
                        of: None,
                        version: 2,
                        at: None,
                        event: json!({ "name": "DISCARDED" }),
                    },
                ),
            ];

            // then: none of them can be made out
            for (case, held, told) in cases {
                assert_eq!(told_about(held.as_ref(), told), None, "{case}");
            }
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

        fn held(version: u64) -> Vec<(IdeaId, Idea)> {
            let idea = Idea {
                version,
                ..an_idea("The loom")
            };

            vec![(idea.id.clone(), idea)]
        }

        fn none_held() -> Vec<(IdeaId, Idea)> {
            Vec::new()
        }

        #[test]
        fn a_change_told_awaits_the_idea_held_at_least_at_its_version() {
            // when: a change is told
            let awaited = awaited(&told(2, json!({ "name": "RETITLED", "title": "The weft" })))
                .expect("a retitle is awaited");

            // then: neither no idea nor the idea before it holds it
            assert!(!awaited.holds(&none_held()));
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
            let awaited =
                awaited(&told(2, json!({ "name": "DISCARDED" }))).expect("a discard is awaited");

            // then: the idea still held does not hold it
            assert!(!awaited.holds(&held(1)));
            // and: the idea gone does
            assert!(awaited.holds(&none_held()));
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
            assert_eq!(awaited(&nameless), None);
            assert_eq!(awaited(&told(2, json!({ "name": "ARCHIVED" }))), None);
        }
    }
}
