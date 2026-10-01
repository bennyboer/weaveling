use boards_core::{BoardEvent, IdeaLink, PositionedIdea, ProjectLink, Size, Spot};
use eventsourcing::Codec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum StoredBoardEvent {
    Started {
        project: String,
    },
    IdeaPinned {
        idea: String,
        at: StoredSpot,
        size: StoredSize,
    },
    IdeaMoved {
        idea: String,
        to: StoredSpot,
    },
    IdeaResized {
        idea: String,
        to: StoredSize,
    },
    IdeaRaised {
        idea: String,
    },
    IdeaUnpinned {
        idea: String,
    },
    Discarded,
    Snapshotted {
        project: String,
        ideas: Vec<StoredPositionedIdea>,
        discarded: bool,
    },
}

#[derive(Serialize, Deserialize)]
struct StoredSpot {
    x: i64,
    y: i64,
}

#[derive(Serialize, Deserialize)]
struct StoredSize {
    width: i64,
    height: i64,
}

#[derive(Serialize, Deserialize)]
struct StoredPositionedIdea {
    idea: String,
    spot: StoredSpot,
    size: StoredSize,
}

pub fn codec() -> Codec<BoardEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredBoardEvent::from(event))
                .expect("a stored board event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredBoardEvent>(body)
                .ok()
                .map(BoardEvent::from)
        },
    }
}

impl From<Spot> for StoredSpot {
    fn from(spot: Spot) -> Self {
        Self {
            x: spot.x,
            y: spot.y,
        }
    }
}

impl From<StoredSpot> for Spot {
    fn from(stored: StoredSpot) -> Self {
        Self {
            x: stored.x,
            y: stored.y,
        }
    }
}

impl From<Size> for StoredSize {
    fn from(size: Size) -> Self {
        Self {
            width: size.width,
            height: size.height,
        }
    }
}

impl From<StoredSize> for Size {
    fn from(stored: StoredSize) -> Self {
        Self {
            width: stored.width,
            height: stored.height,
        }
    }
}

impl From<&PositionedIdea> for StoredPositionedIdea {
    fn from(placed: &PositionedIdea) -> Self {
        Self {
            idea: placed.idea.as_str().to_owned(),
            spot: StoredSpot::from(placed.spot),
            size: StoredSize::from(placed.size),
        }
    }
}

impl From<StoredPositionedIdea> for PositionedIdea {
    fn from(stored: StoredPositionedIdea) -> Self {
        Self {
            idea: IdeaLink::from(stored.idea),
            spot: Spot::from(stored.spot),
            size: Size::from(stored.size),
        }
    }
}

impl From<&BoardEvent> for StoredBoardEvent {
    fn from(event: &BoardEvent) -> Self {
        match event {
            BoardEvent::Started { project } => Self::Started {
                project: project.as_str().to_owned(),
            },
            BoardEvent::IdeaPinned { idea, at, size } => Self::IdeaPinned {
                idea: idea.as_str().to_owned(),
                at: StoredSpot::from(*at),
                size: StoredSize::from(*size),
            },
            BoardEvent::IdeaMoved { idea, to } => Self::IdeaMoved {
                idea: idea.as_str().to_owned(),
                to: StoredSpot::from(*to),
            },
            BoardEvent::IdeaResized { idea, to } => Self::IdeaResized {
                idea: idea.as_str().to_owned(),
                to: StoredSize::from(*to),
            },
            BoardEvent::IdeaRaised { idea } => Self::IdeaRaised {
                idea: idea.as_str().to_owned(),
            },
            BoardEvent::IdeaUnpinned { idea } => Self::IdeaUnpinned {
                idea: idea.as_str().to_owned(),
            },
            BoardEvent::Discarded => Self::Discarded,
            BoardEvent::Snapshotted {
                project,
                ideas,
                discarded,
            } => Self::Snapshotted {
                project: project.as_str().to_owned(),
                ideas: ideas.iter().map(StoredPositionedIdea::from).collect(),
                discarded: *discarded,
            },
        }
    }
}

impl From<StoredBoardEvent> for BoardEvent {
    fn from(stored: StoredBoardEvent) -> Self {
        match stored {
            StoredBoardEvent::Started { project } => Self::Started {
                project: ProjectLink::from(project),
            },
            StoredBoardEvent::IdeaPinned { idea, at, size } => Self::IdeaPinned {
                idea: IdeaLink::from(idea),
                at: Spot::from(at),
                size: Size::from(size),
            },
            StoredBoardEvent::IdeaMoved { idea, to } => Self::IdeaMoved {
                idea: IdeaLink::from(idea),
                to: Spot::from(to),
            },
            StoredBoardEvent::IdeaResized { idea, to } => Self::IdeaResized {
                idea: IdeaLink::from(idea),
                to: Size::from(to),
            },
            StoredBoardEvent::IdeaRaised { idea } => Self::IdeaRaised {
                idea: IdeaLink::from(idea),
            },
            StoredBoardEvent::IdeaUnpinned { idea } => Self::IdeaUnpinned {
                idea: IdeaLink::from(idea),
            },
            StoredBoardEvent::Discarded => Self::Discarded,
            StoredBoardEvent::Snapshotted {
                project,
                ideas,
                discarded,
            } => Self::Snapshotted {
                project: ProjectLink::from(project),
                ideas: ideas.into_iter().map(PositionedIdea::from).collect(),
                discarded,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(event: BoardEvent) -> BoardEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    fn a_spot() -> Spot {
        Spot { x: -40, y: 120 }
    }

    fn a_size() -> Size {
        Size {
            width: 220,
            height: 140,
        }
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        for event in [
            BoardEvent::Started {
                project: ProjectLink::from("project_1"),
            },
            BoardEvent::IdeaPinned {
                idea: IdeaLink::from("idea_1"),
                at: a_spot(),
                size: a_size(),
            },
            BoardEvent::IdeaMoved {
                idea: IdeaLink::from("idea_1"),
                to: a_spot(),
            },
            BoardEvent::IdeaResized {
                idea: IdeaLink::from("idea_1"),
                to: a_size(),
            },
            BoardEvent::IdeaRaised {
                idea: IdeaLink::from("idea_1"),
            },
            BoardEvent::IdeaUnpinned {
                idea: IdeaLink::from("idea_1"),
            },
            BoardEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                ideas: vec![
                    PositionedIdea {
                        idea: IdeaLink::from("idea_1"),
                        spot: a_spot(),
                        size: a_size(),
                    },
                    PositionedIdea {
                        idea: IdeaLink::from("idea_2"),
                        spot: Spot { x: 0, y: 0 },
                        size: a_size(),
                    },
                ],
                discarded: false,
            },
            BoardEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                ideas: Vec::new(),
                discarded: false,
            },
        ] {
            assert_eq!(round_trip(event.clone()), event);
        }
    }

    #[test]
    fn a_snapshot_keeps_its_ideas_in_order() {
        let stacked = BoardEvent::Snapshotted {
            project: ProjectLink::from("project_1"),
            ideas: ["idea_3", "idea_1", "idea_2"]
                .into_iter()
                .map(|named| PositionedIdea {
                    idea: IdeaLink::from(named),
                    spot: a_spot(),
                    size: a_size(),
                })
                .collect(),
            discarded: false,
        };

        assert_eq!(
            round_trip(stacked.clone()),
            stacked,
            "the order of a board's ideas is what stacking means, so it cannot be sorted away"
        );
    }

    #[test]
    fn a_body_written_in_a_shape_we_no_longer_know_reads_as_nothing() {
        let nonsense = serde_json::json!({ "WrittenByAHandFromAnotherAge": { "runes": 3 } });

        assert!(((codec().event)(nonsense)).is_none());
    }

    #[test]
    fn the_stored_shape_names_its_variant() {
        let written = (codec().body)(&BoardEvent::IdeaRaised {
            idea: IdeaLink::from("idea_1"),
        });

        assert_eq!(
            written,
            serde_json::json!({ "IdeaRaised": { "idea": "idea_1" } })
        );
    }
}
