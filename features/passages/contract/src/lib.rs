use serde::{Deserialize, Serialize};

pub const FRAGMENT: &str = "prose";

pub const MORE_TO_SWEEP: &str = "passage.sweep.more";
pub const IDEA_LINKED: &str = "passage.idea.linked";
pub const IDEA_UNLINKED: &str = "passage.idea.unlinked";
pub const DELETED: &str = "passage.deleted";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct IdeaLinkDTO {
    pub passage: String,
    pub idea: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PassageDeletedDTO {
    pub passage: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MoreToSweepDTO {
    pub project: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreatePassageRequest {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PassageDTO {
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
pub struct RetitlePassageRequest {
    pub title: String,
}
