use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct WordCountDTO {
    pub scene: String,
    pub words: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectWordCountsDTO {
    pub counts: Vec<WordCountDTO>,
}
