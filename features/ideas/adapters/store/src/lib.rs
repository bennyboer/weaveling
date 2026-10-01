use eventsourcing::Codec;
use ideas_core::{IdeaEvent, IdeaTitle, PassageLink, ProjectLink};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum StoredIdeaEvent {
    Captured {
        project: String,
        title: String,
    },
    Retitled {
        title: String,
    },
    PassageAttached {
        passage: String,
    },
    Discarded {
        passage: Option<String>,
    },
    Snapshotted {
        project: String,
        title: String,
        passage: Option<String>,
        discarded: bool,
    },
}

pub fn codec() -> Codec<IdeaEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredIdeaEvent::from(event))
                .expect("a stored idea event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredIdeaEvent>(body)
                .ok()
                .and_then(|stored| IdeaEvent::try_from(stored).ok())
        },
    }
}

impl From<&IdeaEvent> for StoredIdeaEvent {
    fn from(event: &IdeaEvent) -> Self {
        match event {
            IdeaEvent::Captured { project, title } => Self::Captured {
                project: project.as_str().to_owned(),
                title: title.as_str().to_owned(),
            },
            IdeaEvent::Retitled(title) => Self::Retitled {
                title: title.as_str().to_owned(),
            },
            IdeaEvent::PassageAttached { passage } => Self::PassageAttached {
                passage: passage.as_str().to_owned(),
            },
            IdeaEvent::Discarded { passage } => Self::Discarded {
                passage: passage.as_ref().map(|link| link.as_str().to_owned()),
            },
            IdeaEvent::Snapshotted {
                project,
                title,
                passage,
                discarded,
            } => Self::Snapshotted {
                project: project.as_str().to_owned(),
                title: title.as_str().to_owned(),
                passage: passage.as_ref().map(|link| link.as_str().to_owned()),
                discarded: *discarded,
            },
        }
    }
}

impl TryFrom<StoredIdeaEvent> for IdeaEvent {
    type Error = ideas_core::InvalidIdeaTitle;

    fn try_from(stored: StoredIdeaEvent) -> Result<Self, Self::Error> {
        Ok(match stored {
            StoredIdeaEvent::Captured { project, title } => Self::Captured {
                project: ProjectLink::from(project),
                title: IdeaTitle::new(&title)?,
            },
            StoredIdeaEvent::Retitled { title } => Self::Retitled(IdeaTitle::new(&title)?),
            StoredIdeaEvent::PassageAttached { passage } => Self::PassageAttached {
                passage: PassageLink::from(passage),
            },
            StoredIdeaEvent::Discarded { passage } => Self::Discarded {
                passage: passage.map(PassageLink::from),
            },
            StoredIdeaEvent::Snapshotted {
                project,
                title,
                passage,
                discarded,
            } => Self::Snapshotted {
                project: ProjectLink::from(project),
                title: IdeaTitle::new(&title)?,
                passage: passage.map(PassageLink::from),
                discarded,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(event: IdeaEvent) -> IdeaEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    fn a_title(saying: &str) -> IdeaTitle {
        IdeaTitle::new(saying).expect("the title should be usable")
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        for event in [
            IdeaEvent::Captured {
                project: ProjectLink::from("project_1"),
                title: a_title("A girl in a wood"),
            },
            IdeaEvent::Retitled(a_title("The fox who lied")),
            IdeaEvent::PassageAttached {
                passage: PassageLink::from("passage_1"),
            },
            IdeaEvent::Discarded { passage: None },
            IdeaEvent::Discarded {
                passage: Some(PassageLink::from("passage_1")),
            },
            IdeaEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                title: a_title("A crown of straw"),
                passage: Some(PassageLink::from("passage_1")),
                discarded: true,
            },
            IdeaEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                title: a_title("Unwritten"),
                passage: None,
                discarded: false,
            },
        ] {
            assert_eq!(round_trip(event.clone()), event);
        }
    }

    #[test]
    fn a_body_written_in_a_shape_we_no_longer_know_reads_as_nothing() {
        let nonsense = serde_json::json!({ "WrittenByAHandFromAnotherAge": { "runes": 3 } });

        assert!(((codec().event)(nonsense)).is_none());
    }

    #[test]
    fn an_untitled_idea_round_trips_because_the_domain_allows_one() {
        assert_eq!(
            round_trip(IdeaEvent::Retitled(IdeaTitle::untitled())),
            IdeaEvent::Retitled(IdeaTitle::untitled())
        );
    }

    #[test]
    fn a_stored_title_the_domain_would_refuse_reads_as_nothing() {
        let controlled = serde_json::json!({ "Retitled": { "title": "a\u{7}b" } });

        assert!(
            ((codec().event)(controlled)).is_none(),
            "a row the domain would refuse must not become an event, or an aggregate could \
             replay into a state its own rules forbid"
        );
    }

    #[test]
    fn the_stored_shape_names_its_variant() {
        let written = (codec().body)(&IdeaEvent::Discarded { passage: None });

        assert_eq!(
            written,
            serde_json::json!({ "Discarded": { "passage": null } })
        );
    }
}
