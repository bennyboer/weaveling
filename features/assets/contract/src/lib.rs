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
        original_size: u64,
        total_size: u64,
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

/// An asset as it is handed out: whose it is, its picture, the widths it can be had in, and every byte kept for it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AssetDTO {
    pub id: String,
    pub version: u64,
    pub owner: OwnerDTO,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub widths: Vec<u32>,
    pub original_size: u64,
    pub total_size: u64,
}

/// What the signed-in account's uploads take, in bytes, and the most they may, none where it is held to no limit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UsageDTO {
    pub used: u64,
    pub most: Option<u64>,
}

/// The limit an operator gives an account of its own, in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountLimitRequest {
    pub most: u64,
}

pub mod refused {
    pub const ID_INVALID: &str = "asset-id-invalid";
    pub const NOT_AN_IMAGE: &str = "asset-not-an-image";
    pub const IMAGE_UNREADABLE: &str = "asset-image-unreadable";
    pub const IMAGE_TOO_LARGE: &str = "asset-image-too-large";
    pub const LIMIT_REACHED: &str = "asset-limit-reached";
    pub const NOT_AN_AUTHOR: &str = "not-an-author";
    pub const NO_ACCOUNT: &str = "asset-no-account";
    pub const NOT_AN_OPERATOR: &str = "not-an-operator";
    pub const LIMIT_UNCOUNTABLE: &str = "asset-limit-uncountable";

    pub const EVERY: [&str; 9] = [
        ID_INVALID,
        NOT_AN_IMAGE,
        IMAGE_UNREADABLE,
        IMAGE_TOO_LARGE,
        LIMIT_REACHED,
        NOT_AN_AUTHOR,
        NO_ACCOUNT,
        NOT_AN_OPERATOR,
        LIMIT_UNCOUNTABLE,
    ];
}

/// The bounds a limit reached may be on, as a refusal names them.
pub mod bound {
    pub const UPLOAD: &str = "upload";
    pub const INSTANCE: &str = "instance";
    pub const ACCOUNT: &str = "account";
    pub const PROJECT: &str = "project";
}

pub mod format {
    pub const JPEG: &str = "jpeg";
    pub const PNG: &str = "png";
    pub const GIF: &str = "gif";
    pub const WEBP: &str = "webp";
}
