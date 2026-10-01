use serde::{Deserialize, Serialize};

pub const FRAGMENT: &str = "prose";

pub const MORE_TO_SWEEP: &str = "passage.sweep.more";

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
    pub text: String,
}
