use serde::{Deserialize, Serialize};

pub const UPLOADED: &str = "asset.uploaded";
pub const DELETED: &str = "asset.deleted";
pub const EVERY_ASSET: &str = "asset.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum AssetEventDTO {
    #[serde(rename = "UPLOADED")]
    Uploaded {
        owner: OwnerDTO,
        format: String,
        width: u32,
        height: u32,
        size: u64,
        kept: u64,
    },
    #[serde(rename = "DELETED")]
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum OwnerDTO {
    Project(String),
    Account(String),
}

pub mod format {
    pub const JPEG: &str = "jpeg";
    pub const PNG: &str = "png";
    pub const GIF: &str = "gif";
    pub const WEBP: &str = "webp";
}
