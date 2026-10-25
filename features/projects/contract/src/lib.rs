use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub quotation: Option<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChangeDetailsRequest {
    pub language: Option<String>,
    #[serde(default)]
    pub quotation: Option<String>,
    pub description: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectDetailsDTO {
    pub id: String,
    pub version: u64,
    pub name: String,
    pub language: Option<String>,
    pub quotation: Option<String>,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

pub const STARTED: &str = "project.started";
pub const RENAMED: &str = "project.renamed";
pub const DETAILS_CHANGED: &str = "project.details.changed";
pub const QUOTATION_CHANGED: &str = "project.quotation.changed";
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
    #[serde(rename = "DETAILS_CHANGED")]
    DetailsChanged {
        language: Option<String>,
        quotation: Option<String>,
        description: String,
    },
    #[serde(rename = "QUOTATION_CHANGED")]
    QuotationChanged { quotation: String },
    #[serde(rename = "DELETED")]
    Deleted,
}

pub mod refused {
    pub const ID_INVALID: &str = "project-id-invalid";
    pub const NAME_BLANK: &str = "project-name-blank";
    pub const NAME_CONTROL_CHARACTER: &str = "project-name-control-character";
    pub const NAME_TOO_LONG: &str = "project-name-too-long";
    pub const LANGUAGE_NOT_A_TAG: &str = "book-language-not-a-tag";
    pub const QUOTATION_UNKNOWN: &str = "quotation-marks-unknown";
    pub const DESCRIPTION_CONTROL_CHARACTER: &str = "project-description-control-character";
    pub const DESCRIPTION_TOO_LONG: &str = "project-description-too-long";
    pub const DELETED: &str = "project-deleted";
    pub const NOT_AN_OWNER: &str = "not-an-owner";

    pub const EVERY: [&str; 10] = [
        ID_INVALID,
        NAME_BLANK,
        NAME_CONTROL_CHARACTER,
        NAME_TOO_LONG,
        LANGUAGE_NOT_A_TAG,
        QUOTATION_UNKNOWN,
        DESCRIPTION_CONTROL_CHARACTER,
        DESCRIPTION_TOO_LONG,
        DELETED,
        NOT_AN_OWNER,
    ];
}
