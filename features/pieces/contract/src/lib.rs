use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CapturePieceRequest {
    pub project: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RetitlePieceRequest {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AttachPassageRequest {
    pub passage: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PieceDTO {
    pub id: String,
    pub version: u64,
    pub project: String,
    pub title: String,
    pub passage: Option<String>,
}
