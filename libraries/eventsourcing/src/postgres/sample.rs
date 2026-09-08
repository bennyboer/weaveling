use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::postgres::Codec;
use crate::testing::sample::{SampleEvent, SampleKind};

#[derive(Serialize, Deserialize)]
enum StoredSample {
    CreatedBeforeDescriptions {
        title: String,
    },
    CreatedBeforeKinds {
        title: String,
        description: String,
    },
    Created {
        title: String,
        description: String,
        kind: StoredKind,
    },
    TitleUpdated(String),
    DescriptionUpdated(String),
    Corrected(String),
    Deleted,
    Snapshotted {
        title: String,
        description: String,
        deleted: bool,
    },
}

#[derive(Serialize, Deserialize)]
enum StoredKind {
    Ordinary,
    Remarkable,
}

pub(crate) fn codec() -> Codec<SampleEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredSample::from(event))
                .expect("a stored sample is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredSample>(body)
                .ok()
                .map(SampleEvent::from)
        },
    }
}

impl From<&SampleEvent> for StoredSample {
    fn from(event: &SampleEvent) -> Self {
        match event {
            SampleEvent::CreatedBeforeDescriptions { title } => Self::CreatedBeforeDescriptions {
                title: title.clone(),
            },
            SampleEvent::CreatedBeforeKinds { title, description } => Self::CreatedBeforeKinds {
                title: title.clone(),
                description: description.clone(),
            },
            SampleEvent::Created {
                title,
                description,
                kind,
            } => Self::Created {
                title: title.clone(),
                description: description.clone(),
                kind: StoredKind::from(*kind),
            },
            SampleEvent::TitleUpdated(title) => Self::TitleUpdated(title.clone()),
            SampleEvent::DescriptionUpdated(description) => {
                Self::DescriptionUpdated(description.clone())
            }
            SampleEvent::Corrected(title) => Self::Corrected(title.clone()),
            SampleEvent::Deleted => Self::Deleted,
            SampleEvent::Snapshotted {
                title,
                description,
                deleted,
            } => Self::Snapshotted {
                title: title.clone(),
                description: description.clone(),
                deleted: *deleted,
            },
        }
    }
}

impl From<StoredSample> for SampleEvent {
    fn from(stored: StoredSample) -> Self {
        match stored {
            StoredSample::CreatedBeforeDescriptions { title } => {
                Self::CreatedBeforeDescriptions { title }
            }
            StoredSample::CreatedBeforeKinds { title, description } => {
                Self::CreatedBeforeKinds { title, description }
            }
            StoredSample::Created {
                title,
                description,
                kind,
            } => Self::Created {
                title,
                description,
                kind: SampleKind::from(kind),
            },
            StoredSample::TitleUpdated(title) => Self::TitleUpdated(title),
            StoredSample::DescriptionUpdated(description) => Self::DescriptionUpdated(description),
            StoredSample::Corrected(title) => Self::Corrected(title),
            StoredSample::Deleted => Self::Deleted,
            StoredSample::Snapshotted {
                title,
                description,
                deleted,
            } => Self::Snapshotted {
                title,
                description,
                deleted,
            },
        }
    }
}

impl From<SampleKind> for StoredKind {
    fn from(kind: SampleKind) -> Self {
        match kind {
            SampleKind::Ordinary => Self::Ordinary,
            SampleKind::Remarkable => Self::Remarkable,
        }
    }
}

impl From<StoredKind> for SampleKind {
    fn from(kind: StoredKind) -> Self {
        match kind {
            StoredKind::Ordinary => Self::Ordinary,
            StoredKind::Remarkable => Self::Remarkable,
        }
    }
}

pub(crate) fn nonsense() -> Value {
    serde_json::json!({ "WrittenByAHandFromAnotherAge": { "runes": 3 } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(event: SampleEvent) -> SampleEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        for event in [
            SampleEvent::CreatedBeforeDescriptions {
                title: "The Loom".to_owned(),
            },
            SampleEvent::CreatedBeforeKinds {
                title: "The Loom".to_owned(),
                description: "A silent machine.".to_owned(),
            },
            SampleEvent::Created {
                title: "The Loom".to_owned(),
                description: "A silent machine.".to_owned(),
                kind: SampleKind::Remarkable,
            },
            SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
            SampleEvent::DescriptionUpdated("It remembers.".to_owned()),
            SampleEvent::Corrected("The Loom".to_owned()),
            SampleEvent::Deleted,
            SampleEvent::Snapshotted {
                title: "The Loom".to_owned(),
                description: "A silent machine.".to_owned(),
                deleted: true,
            },
        ] {
            assert_eq!(round_trip(event.clone()), event);
        }
    }

    #[test]
    fn a_body_written_in_a_shape_we_no_longer_know_reads_as_nothing() {
        assert!(((codec().event)(nonsense())).is_none());
    }

    #[test]
    fn the_stored_shape_names_its_variant_so_a_row_can_be_read_without_the_columns() {
        let written = (codec().body)(&SampleEvent::Deleted);

        assert_eq!(written, serde_json::json!("Deleted"));
    }
}
