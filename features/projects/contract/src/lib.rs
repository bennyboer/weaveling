use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RenameProjectRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectDTO {
    pub id: String,
    pub version: u64,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

pub const STARTED: &str = "project.started";
pub const RENAMED: &str = "project.renamed";
pub const DELETED: &str = "project.deleted";
pub const EVERY_PROJECT: &str = "project.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum ProjectEventDTO {
    #[serde(rename = "STARTED")]
    Started {
        #[serde(rename = "project_name")]
        name: String,
    },
    #[serde(rename = "RENAMED")]
    Renamed {
        #[serde(rename = "project_name")]
        name: String,
    },
    #[serde(rename = "DELETED")]
    Deleted,
}
