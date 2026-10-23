use std::fmt::{self, Display, Formatter};

use scenes_contract::SceneSummaryDTO;

use crate::ideas::model::IdeaId;
use crate::speech;

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
            opening: words::opening_of(&self.text),
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
            "" => speech::say("scene-empty"),
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

pub fn hold(held: &mut Vec<SceneSummary>, summary: SceneSummary) {
    match held.iter_mut().find(|known| known.id == summary.id) {
        Some(known) => *known = summary,
        None => held.push(summary),
    }
}

impl From<SceneSummaryDTO> for SceneSummary {
    fn from(dto: SceneSummaryDTO) -> Self {
        Self {
            id: SceneId::from(dto.id),
            title: dto.title,
            opening: dto.opening,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_summary(id: &str, title: &str) -> SceneSummary {
        SceneSummary {
            id: SceneId::from(id.to_owned()),
            title: title.to_owned(),
            opening: String::new(),
        }
    }

    #[test]
    fn a_scene_not_held_is_added() {
        let mut held = vec![a_summary("scene_1", "The arrival")];

        hold(&mut held, a_summary("scene_2", "The loom"));

        assert_eq!(
            held,
            vec![
                a_summary("scene_1", "The arrival"),
                a_summary("scene_2", "The loom")
            ]
        );
    }

    #[test]
    fn a_scene_held_already_is_replaced_and_not_held_twice() {
        let mut held = vec![a_summary("scene_1", "The arrival")];

        hold(&mut held, a_summary("scene_1", "The departure"));

        assert_eq!(held, vec![a_summary("scene_1", "The departure")]);
    }
}
