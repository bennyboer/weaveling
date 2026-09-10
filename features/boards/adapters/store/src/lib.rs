use boards_core::{BoardEvent, PieceLink, PositionedPiece, ProjectLink, Size, Spot};
use eventsourcing::Codec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum StoredBoardEvent {
    Started {
        project: String,
    },
    PiecePinned {
        piece: String,
        at: StoredSpot,
        size: StoredSize,
    },
    PieceMoved {
        piece: String,
        to: StoredSpot,
    },
    PieceResized {
        piece: String,
        to: StoredSize,
    },
    PieceRaised {
        piece: String,
    },
    PieceUnpinned {
        piece: String,
    },
    Snapshotted {
        project: String,
        pieces: Vec<StoredPositionedPiece>,
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
struct StoredPositionedPiece {
    piece: String,
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

impl From<&PositionedPiece> for StoredPositionedPiece {
    fn from(placed: &PositionedPiece) -> Self {
        Self {
            piece: placed.piece.as_str().to_owned(),
            spot: StoredSpot::from(placed.spot),
            size: StoredSize::from(placed.size),
        }
    }
}

impl From<StoredPositionedPiece> for PositionedPiece {
    fn from(stored: StoredPositionedPiece) -> Self {
        Self {
            piece: PieceLink::from(stored.piece),
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
            BoardEvent::PiecePinned { piece, at, size } => Self::PiecePinned {
                piece: piece.as_str().to_owned(),
                at: StoredSpot::from(*at),
                size: StoredSize::from(*size),
            },
            BoardEvent::PieceMoved { piece, to } => Self::PieceMoved {
                piece: piece.as_str().to_owned(),
                to: StoredSpot::from(*to),
            },
            BoardEvent::PieceResized { piece, to } => Self::PieceResized {
                piece: piece.as_str().to_owned(),
                to: StoredSize::from(*to),
            },
            BoardEvent::PieceRaised { piece } => Self::PieceRaised {
                piece: piece.as_str().to_owned(),
            },
            BoardEvent::PieceUnpinned { piece } => Self::PieceUnpinned {
                piece: piece.as_str().to_owned(),
            },
            BoardEvent::Snapshotted { project, pieces } => Self::Snapshotted {
                project: project.as_str().to_owned(),
                pieces: pieces.iter().map(StoredPositionedPiece::from).collect(),
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
            StoredBoardEvent::PiecePinned { piece, at, size } => Self::PiecePinned {
                piece: PieceLink::from(piece),
                at: Spot::from(at),
                size: Size::from(size),
            },
            StoredBoardEvent::PieceMoved { piece, to } => Self::PieceMoved {
                piece: PieceLink::from(piece),
                to: Spot::from(to),
            },
            StoredBoardEvent::PieceResized { piece, to } => Self::PieceResized {
                piece: PieceLink::from(piece),
                to: Size::from(to),
            },
            StoredBoardEvent::PieceRaised { piece } => Self::PieceRaised {
                piece: PieceLink::from(piece),
            },
            StoredBoardEvent::PieceUnpinned { piece } => Self::PieceUnpinned {
                piece: PieceLink::from(piece),
            },
            StoredBoardEvent::Snapshotted { project, pieces } => Self::Snapshotted {
                project: ProjectLink::from(project),
                pieces: pieces.into_iter().map(PositionedPiece::from).collect(),
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
            BoardEvent::PiecePinned {
                piece: PieceLink::from("piece_1"),
                at: a_spot(),
                size: a_size(),
            },
            BoardEvent::PieceMoved {
                piece: PieceLink::from("piece_1"),
                to: a_spot(),
            },
            BoardEvent::PieceResized {
                piece: PieceLink::from("piece_1"),
                to: a_size(),
            },
            BoardEvent::PieceRaised {
                piece: PieceLink::from("piece_1"),
            },
            BoardEvent::PieceUnpinned {
                piece: PieceLink::from("piece_1"),
            },
            BoardEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                pieces: vec![
                    PositionedPiece {
                        piece: PieceLink::from("piece_1"),
                        spot: a_spot(),
                        size: a_size(),
                    },
                    PositionedPiece {
                        piece: PieceLink::from("piece_2"),
                        spot: Spot { x: 0, y: 0 },
                        size: a_size(),
                    },
                ],
            },
            BoardEvent::Snapshotted {
                project: ProjectLink::from("project_1"),
                pieces: Vec::new(),
            },
        ] {
            assert_eq!(round_trip(event.clone()), event);
        }
    }

    #[test]
    fn a_snapshot_keeps_its_pieces_in_order() {
        let stacked = BoardEvent::Snapshotted {
            project: ProjectLink::from("project_1"),
            pieces: ["piece_3", "piece_1", "piece_2"]
                .into_iter()
                .map(|named| PositionedPiece {
                    piece: PieceLink::from(named),
                    spot: a_spot(),
                    size: a_size(),
                })
                .collect(),
        };

        assert_eq!(
            round_trip(stacked.clone()),
            stacked,
            "the order of a board's pieces is what stacking means, so it cannot be sorted away"
        );
    }

    #[test]
    fn a_body_written_in_a_shape_we_no_longer_know_reads_as_nothing() {
        let nonsense = serde_json::json!({ "WrittenByAHandFromAnotherAge": { "runes": 3 } });

        assert!(((codec().event)(nonsense)).is_none());
    }

    #[test]
    fn the_stored_shape_names_its_variant() {
        let written = (codec().body)(&BoardEvent::PieceRaised {
            piece: PieceLink::from("piece_1"),
        });

        assert_eq!(
            written,
            serde_json::json!({ "PieceRaised": { "piece": "piece_1" } })
        );
    }
}
