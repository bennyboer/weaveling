use std::fmt::{self, Display, Formatter};

use crate::projects::model::ProjectId;

use crate::speech::say;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdeaId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Idea {
    pub id: IdeaId,
    pub version: u64,
    pub project: ProjectId,
    pub title: String,
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
    pub fn shown_as(&self) -> String {
        if self.title.is_empty() {
            say("untitled")
        } else {
            self.title.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn an_idea(title: &str) -> Idea {
        Idea {
            id: IdeaId::from("idea_1".to_owned()),
            version: 1,
            project: ProjectId::from("project_1".to_owned()),
            title: title.to_owned(),
        }
    }

    #[test]
    fn an_idea_is_shown_by_its_title() {
        assert_eq!(an_idea("The loom").shown_as(), "The loom");
    }

    #[test]
    fn an_idea_without_a_title_is_shown_as_untitled() {
        assert_eq!(an_idea("").shown_as(), "Untitled");
    }
}
