use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct WordCountDTO {
    pub scene: String,
    pub words: u64,
}
