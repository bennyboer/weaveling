use eventsourcing::Codec;
use pieces_core::{PassageLink, PieceEvent, PieceTitle, ProjectLink};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum StoredPieceEvent {
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
    Discarded,
    Snapshotted {
        project: String,
        title: String,
        passage: Option<String>,
        discarded: bool,
    },
}

pub fn codec() -> Codec<PieceEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredPieceEvent::from(event))
                .expect("a stored piece event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredPieceEvent>(body)
                .ok()
                .and_then(|stored| PieceEvent::try_from(stored).ok())
        },
    }
}

impl From<&PieceEvent> for StoredPieceEvent {
    fn from(event: &PieceEvent) -> Self {
        match event {
            PieceEvent::Captured { project, title } => Self::Captured {
                project: project.as_str().to_owned(),
                title: title.as_str().to_owned(),
            },
            PieceEvent::Retitled(title) => Self::Retitled {
                title: title.as_str().to_owned(),
            },
            PieceEvent::PassageAttached { passage } => Self::PassageAttached {
                passage: passage.as_str().to_owned(),
            },
            PieceEvent::Discarded => Self::Discarded,
            PieceEvent::Snapshotted {
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

impl TryFrom<StoredPieceEvent> for PieceEvent {
    type Error = pieces_core::InvalidPieceTitle;

    fn try_from(stored: StoredPieceEvent) -> Result<Self, Self::Error> {
        Ok(match stored {
            StoredPieceEvent::Captured { project, title } => Self::Captured {
                project: ProjectLink::from(project),
                title: PieceTitle::new(&title)?,
            },
            StoredPieceEvent::Retitled { title } => Self::Retitled(PieceTitle::new(&title)?),
            StoredPieceEvent::PassageAttached { passage } => Self::PassageAttached {
                passage: PassageLink::from(passage),
            },
            StoredPieceEvent::Discarded => Self::Discarded,
            StoredPieceEvent::Snapshotted {
                project,
                title,
                passage,
                discarded,
            } => Self::Snapshotted {
                project: ProjectLink::from(project),
                title: PieceTitle::new(&title)?,
                passage: passage.map(PassageLink::from),
                discarded,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(event: PieceEvent) -> PieceEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    fn a_title(saying: &str) -> PieceTitle {
        PieceTitle::new(saying).expect("the title should be usable")
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        for event in [
            PieceEvent::Captured {
                project: ProjectLink::from("project_1"),
                title: a_title("A girl in a wood"),
            },
            PieceEvent::Retitled(a_title("The fox who lied")),
            PieceEvent::PassageAttached {
                passage: PassageLink::from("passage_1"),
            },
            PieceEvent::Discarded,
            PieceEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                title: a_title("A crown of straw"),
                passage: Some(PassageLink::from("passage_1")),
                discarded: true,
            },
            PieceEvent::Snapshotted {
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
    fn an_untitled_piece_round_trips_because_the_domain_allows_one() {
        assert_eq!(
            round_trip(PieceEvent::Retitled(PieceTitle::untitled())),
            PieceEvent::Retitled(PieceTitle::untitled())
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
        let written = (codec().body)(&PieceEvent::Discarded);

        assert_eq!(written, serde_json::json!("Discarded"));
    }
}
