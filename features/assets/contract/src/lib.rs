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

/// One of the signed-in account's uploads: whose it is, the project's name where the assets heard it, its picture, every byte kept for it, when it was uploaded, and whether the account may delete it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UploadDTO {
    pub id: String,
    pub owner: OwnerDTO,
    pub project_name: Option<String>,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub total_size: u64,
    pub uploaded_at: String,
    pub deletable: bool,
}

/// A page of the signed-in account's uploads, and where the next goes on from, none after the last.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UploadsDTO {
    pub uploads: Vec<UploadDTO>,
    pub next: Option<String>,
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

/// What the instance holds of every asset, in bytes, and the most it may, none where it is held to no limit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct InstanceStorageDTO {
    pub used: u64,
    pub most: Option<u64>,
}

/// What an account's uploads take, the most they may, and whether that is the account's own limit; with its name and address, none where the assets have not heard them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountStorageDTO {
    pub account: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub own_limit: bool,
}

/// What a project's assets take, the most they may, and its owners' accounts; with its name, none where the assets have not heard it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectStorageDTO {
    pub project: String,
    pub name: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub owners: Vec<String>,
}

/// A page of the accounts holding anything, and where the next goes on from, none after the last.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountsStorageDTO {
    pub accounts: Vec<AccountStorageDTO>,
    pub next: Option<String>,
}

/// A page of the projects holding anything, and where the next goes on from, none after the last.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectsStorageDTO {
    pub projects: Vec<ProjectStorageDTO>,
    pub next: Option<String>,
}

/// How holdings are listed: the most held first, or by id; and an account's uploads: the largest first, or the newest.
pub mod sort {
    pub const MOST_USED: &str = "most-used";
    pub const ID: &str = "id";
    pub const LARGEST: &str = "largest";
    pub const NEWEST: &str = "newest";
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
    pub const SORT_UNKNOWN: &str = "asset-sort-unknown";
    pub const CURSOR_INVALID: &str = "asset-cursor-invalid";

    pub const EVERY: [&str; 11] = [
        ID_INVALID,
        NOT_AN_IMAGE,
        IMAGE_UNREADABLE,
        IMAGE_TOO_LARGE,
        LIMIT_REACHED,
        NOT_AN_AUTHOR,
        NO_ACCOUNT,
        NOT_AN_OPERATOR,
        LIMIT_UNCOUNTABLE,
        SORT_UNKNOWN,
        CURSOR_INVALID,
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
