use eventsourcing::Codec;
use projects_core::{
    BookLanguage, InvalidBookLanguage, InvalidProjectName, InvalidQuotationMarks,
    ProjectDescription, ProjectDetails, ProjectEvent, ProjectName, QuotationMarks,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredProjectEvent {
    Started {
        name: String,
    },
    Renamed {
        name: String,
    },
    DetailsChanged {
        language: Option<String>,
        #[serde(default)]
        quotation: Option<String>,
        description: String,
    },
    QuotationChanged {
        quotation: String,
    },
    Deleted,
    Snapshotted {
        name: String,
        language: Option<String>,
        #[serde(default)]
        quotation: Option<String>,
        description: String,
        #[serde(with = "time::serde::rfc3339")]
        created_at: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        updated_at: OffsetDateTime,
        deleted: bool,
    },
}

pub fn codec() -> Codec<ProjectEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredProjectEvent::from(event))
                .expect("a stored project event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredProjectEvent>(body)
                .ok()
                .and_then(|stored| ProjectEvent::try_from(stored).ok())
        },
    }
}

impl From<&ProjectEvent> for StoredProjectEvent {
    fn from(event: &ProjectEvent) -> Self {
        match event {
            ProjectEvent::Started(name) => Self::Started {
                name: name.as_str().to_owned(),
            },
            ProjectEvent::Renamed(name) => Self::Renamed {
                name: name.as_str().to_owned(),
            },
            ProjectEvent::DetailsChanged(details) => Self::DetailsChanged {
                language: details.language.as_ref().map(ToString::to_string),
                quotation: details.quotation.map(|style| style.name().to_owned()),
                description: details.description.to_string(),
            },
            ProjectEvent::QuotationChanged(style) => Self::QuotationChanged {
                quotation: style.name().to_owned(),
            },
            ProjectEvent::Deleted => Self::Deleted,
            ProjectEvent::Snapshotted {
                name,
                details,
                created_at,
                updated_at,
                deleted,
            } => Self::Snapshotted {
                name: name.as_str().to_owned(),
                language: details.language.as_ref().map(ToString::to_string),
                quotation: details.quotation.map(|style| style.name().to_owned()),
                description: details.description.to_string(),
                created_at: *created_at,
                updated_at: *updated_at,
                deleted: *deleted,
            },
        }
    }
}

#[derive(Debug)]
enum UnreadableProjectEvent {
    Name,
    Language,
    Quotation,
    Description,
}

impl From<InvalidProjectName> for UnreadableProjectEvent {
    fn from(_: InvalidProjectName) -> Self {
        Self::Name
    }
}

impl From<InvalidBookLanguage> for UnreadableProjectEvent {
    fn from(_: InvalidBookLanguage) -> Self {
        Self::Language
    }
}

impl From<InvalidQuotationMarks> for UnreadableProjectEvent {
    fn from(_: InvalidQuotationMarks) -> Self {
        Self::Quotation
    }
}

fn details_of(
    language: Option<String>,
    quotation: Option<String>,
    description: &str,
) -> Result<ProjectDetails, UnreadableProjectEvent> {
    Ok(ProjectDetails {
        language: language.as_deref().map(BookLanguage::new).transpose()?,
        quotation: quotation.as_deref().map(QuotationMarks::new).transpose()?,
        description: ProjectDescription::new(description)
            .map_err(|_| UnreadableProjectEvent::Description)?,
    })
}

impl TryFrom<StoredProjectEvent> for ProjectEvent {
    type Error = UnreadableProjectEvent;

    fn try_from(stored: StoredProjectEvent) -> Result<Self, Self::Error> {
        Ok(match stored {
            StoredProjectEvent::Started { name } => Self::Started(ProjectName::new(&name)?),
            StoredProjectEvent::Renamed { name } => Self::Renamed(ProjectName::new(&name)?),
            StoredProjectEvent::DetailsChanged {
                language,
                quotation,
                description,
            } => Self::DetailsChanged(details_of(language, quotation, &description)?),
            StoredProjectEvent::QuotationChanged { quotation } => {
                Self::QuotationChanged(QuotationMarks::new(&quotation)?)
            }
            StoredProjectEvent::Deleted => Self::Deleted,
            StoredProjectEvent::Snapshotted {
                name,
                language,
                quotation,
                description,
                created_at,
                updated_at,
                deleted,
            } => Self::Snapshotted {
                name: ProjectName::new(&name)?,
                details: details_of(language, quotation, &description)?,
                created_at,
                updated_at,
                deleted,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn round_trip(event: ProjectEvent) -> ProjectEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    fn a_name(saying: &str) -> ProjectName {
        ProjectName::new(saying).expect("the name should be usable")
    }

    fn a_description() -> ProjectDetails {
        ProjectDetails {
            language: Some(BookLanguage::new("de").expect("a plain tag")),
            quotation: Some(QuotationMarks::PointingIn),
            description: ProjectDescription::new(
                "A weaver's daughter.
A war.",
            )
            .expect("a plain description"),
        }
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        for event in [
            ProjectEvent::Started(a_name("The Weaver's Apprentice")),
            ProjectEvent::Renamed(a_name("A crown of straw")),
            ProjectEvent::DetailsChanged(a_description()),
            ProjectEvent::DetailsChanged(ProjectDetails::default()),
            ProjectEvent::QuotationChanged(QuotationMarks::SpacedOut),
            ProjectEvent::Deleted,
            ProjectEvent::Snapshotted {
                name: a_name("The Weaver's Apprentice"),
                details: a_description(),
                created_at: at(1_000),
                updated_at: at(2_000),
                deleted: false,
            },
            ProjectEvent::Snapshotted {
                name: a_name("Abandoned"),
                details: ProjectDetails::default(),
                created_at: at(1_000),
                updated_at: at(2_000),
                deleted: true,
            },
        ] {
            assert_eq!(round_trip(event.clone()), event);
        }
    }

    #[test]
    fn a_snapshots_moments_are_stored_in_a_shape_a_human_can_read() {
        let written = (codec().body)(&ProjectEvent::Snapshotted {
            name: a_name("Readable"),
            details: ProjectDetails::default(),
            created_at: at(1_000),
            updated_at: at(2_000),
            deleted: false,
        });

        assert_eq!(
            written["Snapshotted"]["created_at"],
            serde_json::json!("1970-01-01T00:16:40Z"),
            "a jsonb body is read by people debugging, so the moments are RFC 3339 rather than \
             a count of seconds nobody can decode by eye"
        );
    }

    #[test]
    fn a_stored_language_the_domain_would_refuse_reads_as_nothing() {
        let refused =
            serde_json::json!({ "DetailsChanged": { "language": "deutsch", "description": "" } });

        assert!((codec().event)(refused).is_none());
    }

    #[test]
    fn a_body_written_in_a_shape_we_no_longer_know_reads_as_nothing() {
        let nonsense = serde_json::json!({ "WrittenByAHandFromAnotherAge": { "runes": 3 } });

        assert!(((codec().event)(nonsense)).is_none());
    }

    #[test]
    fn a_stored_name_the_domain_would_refuse_reads_as_nothing() {
        for refused in [
            serde_json::json!({ "Renamed": { "name": "   " } }),
            serde_json::json!({ "Renamed": { "name": "a\u{7}b" } }),
        ] {
            assert!(
                ((codec().event)(refused)).is_none(),
                "a row the domain would refuse must not become an event, or an aggregate could \
                 replay into a state its own rules forbid"
            );
        }
    }

    #[test]
    fn the_stored_shape_names_its_variant() {
        let written = (codec().body)(&ProjectEvent::Deleted);

        assert_eq!(written, serde_json::json!("Deleted"));
    }
}
