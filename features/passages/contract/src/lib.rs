use serde::{Deserialize, Serialize};

pub const FRAGMENT: &str = "prose";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreatePassageRequest {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PassageDTO {
    pub id: String,
    pub project: String,
    pub text: String,
}
