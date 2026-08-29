use std::fmt::{self, Display, Formatter};

use crate::passages::model::PassageId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PieceId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub id: PieceId,
    pub version: u64,
    pub title: String,
    pub passage: Option<PassageId>,
}

impl From<String> for PieceId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for PieceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Piece {
    pub fn shown_as(&self) -> &str {
        if self.title.is_empty() {
            "Untitled"
        } else {
            &self.title
        }
    }
}
