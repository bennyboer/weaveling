use std::fmt::{self, Display, Formatter};

use crate::passages::model::PassageId;
use crate::projects::model::ProjectId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IdeaId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Idea {
    pub id: IdeaId,
    pub version: u64,
    pub project: ProjectId,
    pub title: String,
    pub passage: Option<PassageId>,
}

impl From<String> for IdeaId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for IdeaId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Idea {
    pub fn shown_as(&self) -> &str {
        if self.title.is_empty() {
            "Untitled"
        } else {
            &self.title
        }
    }
}
