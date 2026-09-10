use eventsourcing::Codec;
use outline_core::{OutlineEvent, PieceLink, PlacedSection, ProjectLink, SectionId, SectionTitle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum StoredOutlineEvent {
    Started {
        project: String,
    },
    SectionAdded {
        section: String,
        under: Option<String>,
        after: Option<String>,
        title: String,
    },
    SectionRetitled {
        section: String,
        title: String,
    },
    SectionMoved {
        section: String,
        under: Option<String>,
        after: Option<String>,
    },
    SectionPromoted {
        section: String,
    },
    SectionDemoted {
        section: String,
    },
    SectionRemoved {
        section: String,
    },
    PieceAttached {
        piece: String,
        to: String,
        after: Option<String>,
    },
    PieceDetached {
        piece: String,
    },
    Snapshotted {
        project: String,
        sections: Vec<StoredPlacedSection>,
    },
}

#[derive(Serialize, Deserialize)]
struct StoredPlacedSection {
    section: String,
    parent: Option<String>,
    title: String,
    pieces: Vec<String>,
}

#[derive(Debug)]
pub enum Unreadable {
    Section(ids::InvalidId),
    Title(outline_core::InvalidSectionTitle),
}

pub fn codec() -> Codec<OutlineEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredOutlineEvent::from(event))
                .expect("a stored outline event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredOutlineEvent>(body)
                .ok()
                .and_then(|stored| OutlineEvent::try_from(stored).ok())
        },
    }
}

fn named(section: &SectionId) -> String {
    section.to_string()
}

fn section(text: &str) -> Result<SectionId, Unreadable> {
    text.parse().map_err(Unreadable::Section)
}

fn maybe_section(text: Option<String>) -> Result<Option<SectionId>, Unreadable> {
    text.as_deref().map(section).transpose()
}

fn titled(text: &str) -> Result<SectionTitle, Unreadable> {
    SectionTitle::new(text).map_err(Unreadable::Title)
}

impl From<&PlacedSection> for StoredPlacedSection {
    fn from(placed: &PlacedSection) -> Self {
        Self {
            section: named(&placed.section),
            parent: placed.parent.as_ref().map(named),
            title: placed.title.as_str().to_owned(),
            pieces: placed
                .pieces
                .iter()
                .map(|piece| piece.as_str().to_owned())
                .collect(),
        }
    }
}

impl TryFrom<StoredPlacedSection> for PlacedSection {
    type Error = Unreadable;

    fn try_from(stored: StoredPlacedSection) -> Result<Self, Self::Error> {
        Ok(Self {
            section: section(&stored.section)?,
            parent: maybe_section(stored.parent)?,
            title: titled(&stored.title)?,
            pieces: stored.pieces.into_iter().map(PieceLink::from).collect(),
        })
    }
}

impl From<&OutlineEvent> for StoredOutlineEvent {
    fn from(event: &OutlineEvent) -> Self {
        match event {
            OutlineEvent::Started { project } => Self::Started {
                project: project.as_str().to_owned(),
            },
            OutlineEvent::SectionAdded {
                section,
                under,
                after,
                title,
            } => Self::SectionAdded {
                section: named(section),
                under: under.as_ref().map(named),
                after: after.as_ref().map(named),
                title: title.as_str().to_owned(),
            },
            OutlineEvent::SectionRetitled { section, title } => Self::SectionRetitled {
                section: named(section),
                title: title.as_str().to_owned(),
            },
            OutlineEvent::SectionMoved {
                section,
                under,
                after,
            } => Self::SectionMoved {
                section: named(section),
                under: under.as_ref().map(named),
                after: after.as_ref().map(named),
            },
            OutlineEvent::SectionPromoted { section } => Self::SectionPromoted {
                section: named(section),
            },
            OutlineEvent::SectionDemoted { section } => Self::SectionDemoted {
                section: named(section),
            },
            OutlineEvent::SectionRemoved { section } => Self::SectionRemoved {
                section: named(section),
            },
            OutlineEvent::PieceAttached { piece, to, after } => Self::PieceAttached {
                piece: piece.as_str().to_owned(),
                to: named(to),
                after: after.as_ref().map(|link| link.as_str().to_owned()),
            },
            OutlineEvent::PieceDetached { piece } => Self::PieceDetached {
                piece: piece.as_str().to_owned(),
            },
            OutlineEvent::Snapshotted { project, sections } => Self::Snapshotted {
                project: project.as_str().to_owned(),
                sections: sections.iter().map(StoredPlacedSection::from).collect(),
            },
        }
    }
}

impl TryFrom<StoredOutlineEvent> for OutlineEvent {
    type Error = Unreadable;

    fn try_from(stored: StoredOutlineEvent) -> Result<Self, Self::Error> {
        Ok(match stored {
            StoredOutlineEvent::Started { project } => Self::Started {
                project: ProjectLink::from(project),
            },
            StoredOutlineEvent::SectionAdded {
                section: which,
                under,
                after,
                title,
            } => Self::SectionAdded {
                section: section(&which)?,
                under: maybe_section(under)?,
                after: maybe_section(after)?,
                title: titled(&title)?,
            },
            StoredOutlineEvent::SectionRetitled {
                section: which,
                title,
            } => Self::SectionRetitled {
                section: section(&which)?,
                title: titled(&title)?,
            },
            StoredOutlineEvent::SectionMoved {
                section: which,
                under,
                after,
            } => Self::SectionMoved {
                section: section(&which)?,
                under: maybe_section(under)?,
                after: maybe_section(after)?,
            },
            StoredOutlineEvent::SectionPromoted { section: which } => Self::SectionPromoted {
                section: section(&which)?,
            },
            StoredOutlineEvent::SectionDemoted { section: which } => Self::SectionDemoted {
                section: section(&which)?,
            },
            StoredOutlineEvent::SectionRemoved { section: which } => Self::SectionRemoved {
                section: section(&which)?,
            },
            StoredOutlineEvent::PieceAttached { piece, to, after } => Self::PieceAttached {
                piece: PieceLink::from(piece),
                to: section(&to)?,
                after: after.map(PieceLink::from),
            },
            StoredOutlineEvent::PieceDetached { piece } => Self::PieceDetached {
                piece: PieceLink::from(piece),
            },
            StoredOutlineEvent::Snapshotted { project, sections } => Self::Snapshotted {
                project: ProjectLink::from(project),
                sections: sections
                    .into_iter()
                    .map(PlacedSection::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ids::OffsetDateTime;

    fn round_trip(event: OutlineEvent) -> OutlineEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    fn a_section(seconds: i64) -> SectionId {
        SectionId::generate(OffsetDateTime::UNIX_EPOCH + ids::Duration::seconds(seconds))
    }

    fn a_title(saying: &str) -> SectionTitle {
        SectionTitle::new(saying).expect("the title should be usable")
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        let part = a_section(1_000);
        let chapter = a_section(2_000);

        for event in [
            OutlineEvent::Started {
                project: ProjectLink::from("project_1"),
            },
            OutlineEvent::SectionAdded {
                section: chapter,
                under: Some(part),
                after: None,
                title: a_title("Chapter One"),
            },
            OutlineEvent::SectionAdded {
                section: part,
                under: None,
                after: Some(chapter),
                title: a_title("Part One"),
            },
            OutlineEvent::SectionRetitled {
                section: chapter,
                title: a_title("Chapter Two"),
            },
            OutlineEvent::SectionMoved {
                section: chapter,
                under: Some(part),
                after: Some(part),
            },
            OutlineEvent::SectionPromoted { section: chapter },
            OutlineEvent::SectionDemoted { section: chapter },
            OutlineEvent::SectionRemoved { section: chapter },
            OutlineEvent::PieceAttached {
                piece: PieceLink::from("piece_1"),
                to: chapter,
                after: Some(PieceLink::from("piece_2")),
            },
            OutlineEvent::PieceAttached {
                piece: PieceLink::from("piece_1"),
                to: chapter,
                after: None,
            },
            OutlineEvent::PieceDetached {
                piece: PieceLink::from("piece_1"),
            },
            OutlineEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                sections: vec![
                    PlacedSection {
                        section: part,
                        parent: None,
                        title: a_title("Part One"),
                        pieces: Vec::new(),
                    },
                    PlacedSection {
                        section: chapter,
                        parent: Some(part),
                        title: a_title("Chapter One"),
                        pieces: vec![PieceLink::from("piece_1"), PieceLink::from("piece_2")],
                    },
                ],
            },
        ] {
            assert_eq!(round_trip(event.clone()), event);
        }
    }

    #[test]
    fn a_snapshot_keeps_the_reading_order_of_its_sections_and_their_pieces() {
        let shape = OutlineEvent::Snapshotted {
            project: ProjectLink::from("project_1"),
            sections: (1..=4)
                .map(|nth| PlacedSection {
                    section: a_section(nth * 1_000),
                    parent: None,
                    title: a_title(&format!("Section {nth}")),
                    pieces: vec![
                        PieceLink::from(format!("piece_{}b", nth)),
                        PieceLink::from(format!("piece_{}a", nth)),
                    ],
                })
                .collect(),
        };

        assert_eq!(
            round_trip(shape.clone()),
            shape,
            "the outline is the book's order, so nothing about it may be reordered in a column"
        );
    }

    #[test]
    fn a_body_written_in_a_shape_we_no_longer_know_reads_as_nothing() {
        let nonsense = serde_json::json!({ "WrittenByAHandFromAnotherAge": { "runes": 3 } });

        assert!(((codec().event)(nonsense)).is_none());
    }

    #[test]
    fn a_stored_section_that_is_not_an_id_reads_as_nothing() {
        let bent = serde_json::json!({ "SectionPromoted": { "section": "not_a_section" } });

        assert!(
            ((codec().event)(bent)).is_none(),
            "an id the domain cannot parse must not become an event"
        );
    }

    #[test]
    fn an_untitled_section_round_trips_because_the_domain_allows_one() {
        let bare = OutlineEvent::SectionAdded {
            section: a_section(1_000),
            under: None,
            after: None,
            title: a_title(""),
        };

        assert_eq!(
            round_trip(bare.clone()),
            bare,
            "a section is added untitled and named afterwards, so blank has to survive storage"
        );
    }

    #[test]
    fn a_stored_title_the_domain_would_refuse_reads_as_nothing() {
        let controlled = serde_json::json!({
            "SectionRetitled": { "section": a_section(1_000).to_string(), "title": "a\u{7}b" }
        });

        assert!(
            ((codec().event)(controlled)).is_none(),
            "a row the domain would refuse must not become an event, or an aggregate could \
             replay into a state its own rules forbid"
        );
    }
}
