use std::fmt::{self, Display, Formatter};

use indexmap::IndexMap;

use eventsourcing::{
    Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName, Version,
};
use thiserror::Error;

use crate::id::BoardId;
use crate::spot::Spot;

pub const KIND: AggregateType = AggregateType::of("board");

impl From<&BoardId> for AggregateId {
    fn from(id: &BoardId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const STARTED: EventName = EventName::of("STARTED");
const PIECE_PINNED: EventName = EventName::of("PIECE_PINNED");
const PIECE_MOVED: EventName = EventName::of("PIECE_MOVED");
const PIECE_UNPINNED: EventName = EventName::of("PIECE_UNPINNED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PieceLink(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedPiece {
    pub piece: PieceLink,
    pub spot: Spot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardCommand {
    Start { project: ProjectLink },
    Pin { piece: PieceLink, at: Spot },
    Move { piece: PieceLink, to: Spot },
    Unpin { piece: PieceLink },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardEvent {
    Started {
        project: ProjectLink,
    },
    PiecePinned {
        piece: PieceLink,
        at: Spot,
    },
    PieceMoved {
        piece: PieceLink,
        to: Spot,
    },
    PieceUnpinned {
        piece: PieceLink,
    },
    Snapshotted {
        project: ProjectLink,
        pieces: Vec<PositionedPiece>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    project: ProjectLink,
    pieces: IndexMap<PieceLink, Spot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BoardError {
    #[error("a board must be started before anything can be pinned to it")]
    NotStartedYet,
    #[error("a board cannot be started twice")]
    AlreadyStarted,
    #[error("this piece is already on the board")]
    AlreadyPinned,
    #[error("this piece is not on the board")]
    NotPinned,
}

impl ProjectLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProjectLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for ProjectLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for ProjectLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl PieceLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PieceLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for PieceLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for PieceLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Board {
    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn pieces(&self) -> Vec<PositionedPiece> {
        self.pieces
            .iter()
            .map(|(piece, spot)| PositionedPiece {
                piece: piece.clone(),
                spot: *spot,
            })
            .collect()
    }

    pub fn spot_of(&self, piece: &PieceLink) -> Option<Spot> {
        self.pieces.get(piece).copied()
    }

    fn pin(&mut self, piece: &PieceLink, at: Spot) {
        self.pieces.insert(piece.clone(), at);
    }

    fn shift(&mut self, piece: &PieceLink, to: Spot) {
        if let Some(spot) = self.pieces.get_mut(piece) {
            *spot = to;
        }
    }

    fn unpin(&mut self, piece: &PieceLink) {
        self.pieces.shift_remove(piece);
    }

    fn holding(pieces: &[PositionedPiece]) -> IndexMap<PieceLink, Spot> {
        pieces
            .iter()
            .map(|positioned| (positioned.piece.clone(), positioned.spot))
            .collect()
    }
}

impl Event for BoardEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Started { .. } => STARTED,
            Self::PiecePinned { .. } => PIECE_PINNED,
            Self::PieceMoved { .. } => PIECE_MOVED,
            Self::PieceUnpinned { .. } => PIECE_UNPINNED,
            Self::Snapshotted { .. } => SNAPSHOTTED,
        }
    }

    fn version(&self) -> Version {
        Version::ZERO
    }

    fn is_snapshot(&self) -> bool {
        matches!(self, Self::Snapshotted { .. })
    }
}

impl Aggregate for Board {
    type Command = BoardCommand;
    type Event = BoardEvent;
    type Error = BoardError;

    const KIND: AggregateType = KIND;

    fn begin(command: BoardCommand, _agent: &Agent) -> Result<Vec<BoardEvent>, BoardError> {
        match command {
            BoardCommand::Start { project } => Ok(vec![BoardEvent::Started { project }]),
            _ => Err(BoardError::NotStartedYet),
        }
    }

    fn from_first(event: &BoardEvent, _metadata: &EventMetadata) -> Option<Self> {
        match event {
            BoardEvent::Started { project } => Some(Self {
                project: project.clone(),
                pieces: IndexMap::new(),
            }),
            BoardEvent::Snapshotted { project, pieces } => Some(Self {
                project: project.clone(),
                pieces: Self::holding(pieces),
            }),
            _ => None,
        }
    }

    fn decide(&self, command: BoardCommand, _agent: &Agent) -> Result<Vec<BoardEvent>, BoardError> {
        match command {
            BoardCommand::Start { .. } => Err(BoardError::AlreadyStarted),
            BoardCommand::Pin { piece, at } => {
                if self.spot_of(&piece).is_some() {
                    return Err(BoardError::AlreadyPinned);
                }

                Ok(vec![BoardEvent::PiecePinned { piece, at }])
            }
            BoardCommand::Move { piece, to } => match self.spot_of(&piece) {
                None => Err(BoardError::NotPinned),
                Some(already) if already == to => Ok(vec![]),
                Some(_) => Ok(vec![BoardEvent::PieceMoved { piece, to }]),
            },
            BoardCommand::Unpin { piece } => {
                if self.spot_of(&piece).is_none() {
                    return Err(BoardError::NotPinned);
                }

                Ok(vec![BoardEvent::PieceUnpinned { piece }])
            }
        }
    }

    fn apply(&mut self, event: &BoardEvent, _metadata: &EventMetadata) {
        match event {
            BoardEvent::Started { .. } => {}
            BoardEvent::PiecePinned { piece, at } => self.pin(piece, *at),
            BoardEvent::PieceMoved { piece, to } => self.shift(piece, *to),
            BoardEvent::PieceUnpinned { piece } => self.unpin(piece),
            BoardEvent::Snapshotted { project, pieces } => {
                self.project = project.clone();
                self.pieces = Self::holding(pieces);
            }
        }
    }

    fn snapshot(&self) -> BoardEvent {
        BoardEvent::Snapshotted {
            project: self.project.clone(),
            pieces: self.pieces(),
        }
    }
}
