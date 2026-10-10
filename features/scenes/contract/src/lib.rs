use serde::{Deserialize, Serialize};

pub const FRAGMENT: &str = "prose";

pub const MORE_TO_SWEEP: &str = "scene.sweep.more";
pub const IDEA_LINKED: &str = "scene.idea.linked";
pub const IDEA_UNLINKED: &str = "scene.idea.unlinked";
pub const DELETED: &str = "scene.deleted";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct IdeaLinkDTO {
    pub scene: String,
    pub idea: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneDeletedDTO {
    pub scene: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MoreToSweepDTO {
    pub project: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateSceneRequest {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneDTO {
    pub id: String,
    pub project: String,
    pub title: String,
    pub ideas: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LinkIdeaRequest {
    pub idea: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RetitleSceneRequest {
    pub title: String,
}
