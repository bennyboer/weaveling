use std::fmt::{self, Display, Formatter};

use words::opening_of;

use crate::ideas::model::IdeaId;
use crate::speech::say;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SceneId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    pub id: SceneId,
    pub title: String,
    pub ideas: Vec<IdeaId>,
    pub text: String,
}

impl Scene {
    pub fn summarized(&self) -> SceneSummary {
        SceneSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            opening: opening_of(&self.text),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneSummary {
    pub id: SceneId,
    pub title: String,
    pub opening: String,
}

impl SceneSummary {
    pub fn shown_as(&self) -> String {
        if !self.title.trim().is_empty() {
            return self.title.clone();
        }

        match self.opening.as_str() {
            "" => say("scene-empty"),
            opening => opening.to_owned(),
        }
    }
}

impl SceneId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SceneId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for SceneId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
