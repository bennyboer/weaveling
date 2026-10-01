use std::fmt::{self, Display, Formatter};

use indexmap::IndexMap;

use eventsourcing::{
    Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName, Version,
};
use thiserror::Error;

use crate::id::BoardId;
use crate::size::Size;
use crate::spot::Spot;

pub const KIND: AggregateType = AggregateType::of("board");

impl From<&BoardId> for AggregateId {
    fn from(id: &BoardId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const STARTED: EventName = EventName::of("STARTED");
const IDEA_PINNED: EventName = EventName::of("IDEA_PINNED");
const IDEA_MOVED: EventName = EventName::of("IDEA_MOVED");
const IDEA_RESIZED: EventName = EventName::of("IDEA_RESIZED");
const IDEA_RAISED: EventName = EventName::of("IDEA_RAISED");
const IDEA_UNPINNED: EventName = EventName::of("IDEA_UNPINNED");
const DISCARDED: EventName = EventName::of("DISCARDED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdeaLink(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub spot: Spot,
    pub size: Size,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedIdea {
    pub idea: IdeaLink,
    pub spot: Spot,
    pub size: Size,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardCommand {
    Start {
        project: ProjectLink,
    },
    Pin {
        idea: IdeaLink,
        at: Spot,
        size: Size,
    },
    Reshape {
        idea: IdeaLink,
        to: Option<Spot>,
        size: Option<Size>,
    },
    Unpin {
        idea: IdeaLink,
    },
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardEvent {
    Started {
        project: ProjectLink,
    },
    IdeaPinned {
        idea: IdeaLink,
        at: Spot,
        size: Size,
    },
    IdeaMoved {
        idea: IdeaLink,
        to: Spot,
    },
    IdeaResized {
        idea: IdeaLink,
        to: Size,
    },
    IdeaRaised {
        idea: IdeaLink,
    },
    IdeaUnpinned {
        idea: IdeaLink,
    },
    Discarded,
    Snapshotted {
        project: ProjectLink,
        ideas: Vec<PositionedIdea>,
        discarded: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    project: ProjectLink,
    ideas: IndexMap<IdeaLink, Placement>,
    discarded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BoardError {
    #[error("a board must be started before anything can be pinned to it")]
    NotStartedYet,
    #[error("a board cannot be started twice")]
    AlreadyStarted,
    #[error("this idea is already on the board")]
    AlreadyPinned,
    #[error("this idea is not on the board")]
    NotPinned,
    #[error("a card must have width and height")]
    Shapeless,
    #[error("a discarded board accepts no changes")]
    Discarded,
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

impl IdeaLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for IdeaLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for IdeaLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for IdeaLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Board {
    pub fn is_discarded(&self) -> bool {
        self.discarded
    }

    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn ideas(&self) -> Vec<PositionedIdea> {
        self.ideas
            .iter()
            .map(|(idea, placement)| PositionedIdea {
                idea: idea.clone(),
                spot: placement.spot,
                size: placement.size,
            })
            .collect()
    }

    pub fn placement_of(&self, idea: &IdeaLink) -> Option<Placement> {
        self.ideas.get(idea).copied()
    }

    pub fn spot_of(&self, idea: &IdeaLink) -> Option<Spot> {
        self.placement_of(idea).map(|placement| placement.spot)
    }

    pub fn size_of(&self, idea: &IdeaLink) -> Option<Size> {
        self.placement_of(idea).map(|placement| placement.size)
    }

    fn pin(&mut self, idea: &IdeaLink, at: Spot, size: Size) {
        self.ideas
            .insert(idea.clone(), Placement { spot: at, size });
    }

    fn shift(&mut self, idea: &IdeaLink, to: Spot) {
        if let Some(placement) = self.ideas.get_mut(idea) {
            placement.spot = to;
        }
    }

    fn resize(&mut self, idea: &IdeaLink, to: Size) {
        if let Some(placement) = self.ideas.get_mut(idea) {
            placement.size = to;
        }
    }

    fn raise(&mut self, idea: &IdeaLink) {
        if let Some(placement) = self.ideas.shift_remove(idea) {
            self.ideas.insert(idea.clone(), placement);
        }
    }

    fn is_topmost(&self, idea: &IdeaLink) -> bool {
        self.ideas.last().map(|(held, _)| held) == Some(idea)
    }

    fn unpin(&mut self, idea: &IdeaLink) {
        self.ideas.shift_remove(idea);
    }

    fn holding(ideas: &[PositionedIdea]) -> IndexMap<IdeaLink, Placement> {
        ideas
            .iter()
            .map(|positioned| {
                (
                    positioned.idea.clone(),
                    Placement {
                        spot: positioned.spot,
                        size: positioned.size,
                    },
                )
            })
            .collect()
    }
}

impl Event for BoardEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Started { .. } => STARTED,
            Self::IdeaPinned { .. } => IDEA_PINNED,
            Self::IdeaMoved { .. } => IDEA_MOVED,
            Self::IdeaResized { .. } => IDEA_RESIZED,
            Self::IdeaRaised { .. } => IDEA_RAISED,
            Self::IdeaUnpinned { .. } => IDEA_UNPINNED,
            Self::Discarded => DISCARDED,
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
                ideas: IndexMap::new(),
                discarded: false,
            }),
            BoardEvent::Snapshotted {
                project,
                ideas,
                discarded,
            } => Some(Self {
                project: project.clone(),
                ideas: Self::holding(ideas),
                discarded: *discarded,
            }),
            _ => None,
        }
    }

    fn decide(&self, command: BoardCommand, _agent: &Agent) -> Result<Vec<BoardEvent>, BoardError> {
        if self.discarded {
            return Err(BoardError::Discarded);
        }

        match command {
            BoardCommand::Start { .. } => Err(BoardError::AlreadyStarted),
            BoardCommand::Discard => Ok(vec![BoardEvent::Discarded]),
            BoardCommand::Pin { idea, at, size } => {
                if self.placement_of(&idea).is_some() {
                    return Err(BoardError::AlreadyPinned);
                }

                if !size.has_extent() {
                    return Err(BoardError::Shapeless);
                }

                Ok(vec![BoardEvent::IdeaPinned { idea, at, size }])
            }
            BoardCommand::Reshape { idea, to, size } => {
                let Some(already) = self.placement_of(&idea) else {
                    return Err(BoardError::NotPinned);
                };

                if size.is_some_and(|size| !size.has_extent()) {
                    return Err(BoardError::Shapeless);
                }

                let mut happened = Vec::new();

                if let Some(to) = to.filter(|to| *to != already.spot) {
                    happened.push(BoardEvent::IdeaMoved {
                        idea: idea.clone(),
                        to,
                    });

                    if !self.is_topmost(&idea) {
                        happened.push(BoardEvent::IdeaRaised { idea: idea.clone() });
                    }
                }

                if let Some(to) = size.filter(|size| *size != already.size) {
                    happened.push(BoardEvent::IdeaResized { idea, to });
                }

                Ok(happened)
            }
            BoardCommand::Unpin { idea } => {
                if self.placement_of(&idea).is_none() {
                    return Err(BoardError::NotPinned);
                }

                Ok(vec![BoardEvent::IdeaUnpinned { idea }])
            }
        }
    }

    fn apply(&mut self, event: &BoardEvent, _metadata: &EventMetadata) {
        match event {
            BoardEvent::Started { .. } => {}
            BoardEvent::IdeaPinned { idea, at, size } => self.pin(idea, *at, *size),
            BoardEvent::IdeaMoved { idea, to } => self.shift(idea, *to),
            BoardEvent::IdeaResized { idea, to } => self.resize(idea, *to),
            BoardEvent::IdeaRaised { idea } => self.raise(idea),
            BoardEvent::IdeaUnpinned { idea } => self.unpin(idea),
            BoardEvent::Discarded => self.discarded = true,
            BoardEvent::Snapshotted {
                project,
                ideas,
                discarded,
            } => {
                self.project = project.clone();
                self.ideas = Self::holding(ideas);
                self.discarded = *discarded;
            }
        }
    }

    fn snapshot(&self) -> BoardEvent {
        BoardEvent::Snapshotted {
            project: self.project.clone(),
            ideas: self.ideas(),
            discarded: self.discarded,
        }
    }
}
