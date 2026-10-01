use std::fmt::{self, Display, Formatter};

use crate::ideas::model::IdeaId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoardId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub spot: Spot,
    pub size: Size,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionedIdea {
    pub idea: IdeaId,
    pub spot: Spot,
    pub size: Size,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub id: BoardId,
    pub version: u64,
    pub ideas: Vec<PositionedIdea>,
}

impl From<String> for BoardId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for BoardId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Board {
    pub fn holds(&self, idea: &IdeaId) -> bool {
        self.ideas.iter().any(|held| &held.idea == idea)
    }
}
