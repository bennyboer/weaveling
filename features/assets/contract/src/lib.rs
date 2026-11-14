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

/// An asset as it is handed out.
/// It holds its owner, its picture, the widths it is available in, and all bytes stored for it.
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
    #[serde(default)]
    pub uses: Vec<AssetUseDTO>,
}

/// What uses an asset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum HolderDTO {
    Account(String),
}

/// One use of an asset: what it is used for, and by what.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AssetUseDTO {
    pub purpose: String,
    pub by: HolderDTO,
}

/// What an asset can be used for.
pub mod purpose {
    pub const ACCOUNT_PICTURE: &str = "account-picture";
}

/// One of the signed-in account's uploads.
/// It holds its owner, its picture, all bytes stored for it, when it was uploaded, and whether the account may delete it.
/// It also holds the project's name, if the assets feature heard it.
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
    #[serde(default)]
    pub uses: Vec<AssetUseDTO>,
}

/// A page of the signed-in account's uploads, and the cursor for the next page.
/// The cursor is missing after the last page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UploadsDTO {
    pub uploads: Vec<UploadDTO>,
    pub next: Option<String>,
}

/// How many bytes the signed-in account's uploads use, and their limit.
/// The limit is missing if there is none.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UsageDTO {
    pub used: u64,
    pub most: Option<u64>,
}

/// A storage limit an operator gives to a single account, in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountLimitRequest {
    pub most: u64,
}

/// How many bytes the whole instance stores across all assets, and its limit.
/// The limit is missing if there is none.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct InstanceStorageDTO {
    pub used: u64,
    pub most: Option<u64>,
}

/// How many bytes an account's uploads use, their limit, and whether that limit is the account's own.
/// It also holds the account's name and email.
/// Both are missing if the assets feature never heard them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountStorageDTO {
    pub account: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub own_limit: bool,
}

/// How many bytes a project's assets use, their limit, and the accounts of its owners.
/// It also holds the project's name, which is missing if the assets feature never heard it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectStorageDTO {
    pub project: String,
    pub name: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub owners: Vec<ProjectOwnerDTO>,
}

/// An owner of a project, with their name and email, both missing if the assets feature never heard them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectOwnerDTO {
    pub account: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// A page of the accounts that store anything, and the cursor for the next page.
/// The cursor is missing after the last page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountsStorageDTO {
    pub accounts: Vec<AccountStorageDTO>,
    pub next: Option<String>,
}

/// A page of the projects that store anything, and the cursor for the next page.
/// The cursor is missing after the last page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectsStorageDTO {
    pub projects: Vec<ProjectStorageDTO>,
    pub next: Option<String>,
}

/// How lists are sorted.
/// Holdings are listed with the largest use first, or by id.
/// An account's uploads are listed with the largest first, or the newest first.
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
    pub const IN_USE: &str = "asset-in-use";

    pub const EVERY: [&str; 12] = [
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
        IN_USE,
    ];
}

/// The bounds whose limit can be reached.
/// A refusal names one of them.
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
