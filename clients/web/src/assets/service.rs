use assets_contract::{
    AccountLimitRequest, AccountsStorageDTO, InstanceStorageDTO, OwnerDTO, ProjectsStorageDTO,
    UploadDTO, UploadsDTO, UsageDTO, asset_type, bound, purpose, refused, sort,
};
use assets_contract::{AssetDTO, AssetUseDTO, HolderDTO, ProjectAssetsDTO, UploadLimitsDTO};
use gloo_net::http::Request;
use js_sys::Uint8Array;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortSignal, File};

use crate::http;
use crate::http::{ApiError, Particular, Refusal};
use crate::scenes::model::SceneId;

const ASSETS: &str = "/api/assets";
const USAGE: &str = "/api/assets/usage";
const LIMITS: &str = "/api/assets/limits";
const INSTANCE: &str = "/api/assets/storage";
const ACCOUNTS: &str = "/api/assets/storage/accounts";
const PROJECTS: &str = "/api/assets/storage/projects";
const ASSET: &str = "asset";

/// The order of one's uploads: the newest first, or the largest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadsSorted {
    Newest,
    Largest,
}

impl UploadsSorted {
    pub fn named(self) -> &'static str {
        match self {
            Self::Newest => sort::NEWEST,
            Self::Largest => sort::LARGEST,
        }
    }

    pub fn of(named: &str) -> Option<Self> {
        match named {
            sort::NEWEST => Some(Self::Newest),
            sort::LARGEST => Some(Self::Largest),
            _ => None,
        }
    }
}

/// What an upload is used for.
/// `Other` stands for a purpose this client does not know yet, told by a newer server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    AccountPicture,
    SceneImage,
    Other,
}

impl Purpose {
    fn of(named: &str) -> Self {
        match named {
            purpose::ACCOUNT_PICTURE => Self::AccountPicture,
            purpose::SCENE_IMAGE => Self::SceneImage,
            _ => Self::Other,
        }
    }
}

/// What holds an upload in use: an account, as its picture, or a scene, in its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holder {
    Account(String),
    Scene(SceneId),
}

/// One use of an upload: what it is used for, and what uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    pub purpose: Purpose,
    pub by: Holder,
}

/// One of the signed-in account's uploads.
/// `project` is missing for the account's own images, and `project_name` also when the name is not known.
/// `uses` says what the upload is used for. An upload in use cannot be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub id: String,
    pub content_type: String,
    pub project: Option<String>,
    pub project_name: Option<String>,
    pub total_size: u64,
    pub uploaded_at: OffsetDateTime,
    pub deletable: bool,
    pub uses: Vec<Use>,
}

/// One page of uploads, and the cursor for the next page, if there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadPage {
    pub uploads: Vec<Upload>,
    pub next: Option<String>,
}

/// How much storage the account's uploads use, and its limit, if it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub used: u64,
    pub most: Option<u64>,
}

/// How much storage one account uses, as an operator sees it.
/// `name` and `email` are missing if the assets feature never heard them.
/// `most` is the account's own limit if `own_limit`, or else the default limit for accounts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStorage {
    pub account: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub own_limit: bool,
}

/// One page of accounts, the ones using the most first, and the cursor for the next page, if there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountsPage {
    pub accounts: Vec<AccountStorage>,
    pub next: Option<String>,
}

/// An owner of a project, with name and email if the assets feature heard them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectOwner {
    pub account: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// How much storage one project uses, its limit, and its owners, as an operator sees it.
/// `name` is missing if the assets feature never heard it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectStorage {
    pub project: String,
    pub name: Option<String>,
    pub used: u64,
    pub most: Option<u64>,
    pub owners: Vec<ProjectOwner>,
}

/// One page of projects, the ones using the most first, and the cursor for the next page, if there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectsPage {
    pub projects: Vec<ProjectStorage>,
    pub next: Option<String>,
}

fn accounts_listing(search: &str, after: Option<&str>) -> String {
    storage_listing(ACCOUNTS, search, after)
}

fn projects_listing(search: &str, after: Option<&str>) -> String {
    storage_listing(PROJECTS, search, after)
}

fn storage_listing(listing: &str, search: &str, after: Option<&str>) -> String {
    addressed(
        listing,
        &[
            ("sort", Some(sort::MOST_USED.to_owned())),
            ("after", after.map(str::to_owned)),
            (
                "search",
                Some(search.trim().to_owned()).filter(|term| !term.is_empty()),
            ),
        ],
    )
}

/// Builds an address from a path and what it asks, leaving out what is not asked and encoding every value.
fn addressed(path: &str, asked: &[(&str, Option<String>)]) -> String {
    let asked: Vec<String> = asked
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_deref()
                .map(|value| format!("{name}={}", encoded(value)))
        })
        .collect();

    match asked.is_empty() {
        true => path.to_owned(),
        false => format!("{path}?{}", asked.join("&")),
    }
}

/// Percent-encodes a term for an address, keeping only letters, digits and `-_.~` as they are.
fn encoded(term: &str) -> String {
    term.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn limit_of(account: &str) -> String {
    format!("{ASSETS}/accounts/{account}/limit")
}

/// What kind of asset a listing is asked for: the first part of a media type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    Image,
}

impl AssetType {
    fn named(self) -> &'static str {
        match self {
            Self::Image => asset_type::IMAGE,
        }
    }
}

fn listing(sorted: UploadsSorted, of_type: Option<AssetType>, after: Option<&str>) -> String {
    addressed(
        ASSETS,
        &[
            ("sort", Some(sorted.named().to_owned())),
            ("type", of_type.map(|of_type| of_type.named().to_owned())),
            ("after", after.map(str::to_owned)),
        ],
    )
}

/// Returns the address of an image's variant at least as wide as asked.
pub fn content(id: &str, width: u32) -> String {
    format!("{ASSETS}/{id}/content?width={width}")
}

fn read(told: UploadDTO) -> Result<Upload, ApiError> {
    Ok(Upload {
        project: match told.owner {
            OwnerDTO::Project(project) => Some(project),
            OwnerDTO::Account(_) => None,
        },
        project_name: told.project_name,
        total_size: told.total_size,
        uploaded_at: OffsetDateTime::parse(&told.uploaded_at, &Rfc3339)
            .map_err(|_| ApiError::Unexpected)?,
        deletable: told.deletable,
        content_type: told.content_type,
        uses: told.uses.into_iter().map(use_read).collect(),
        id: told.id,
    })
}

fn use_read(used: AssetUseDTO) -> Use {
    Use {
        purpose: Purpose::of(&used.purpose),
        by: match used.by {
            HolderDTO::Account(account) => Holder::Account(account),
            HolderDTO::Scene(scene) => Holder::Scene(SceneId::from(scene)),
        },
    }
}

/// Lists everything the signed-in account uploaded, a page at a time.
/// Given a type, it lists only the uploads of that type.
pub async fn uploads(
    sorted: UploadsSorted,
    of_type: Option<AssetType>,
    after: Option<&str>,
) -> Result<UploadPage, ApiError> {
    let page: UploadsDTO = http::parsed(
        http::sent(Request::get(&listing(sorted, of_type, after))).await?,
        ASSET,
    )
    .await?;

    Ok(UploadPage {
        uploads: page
            .uploads
            .into_iter()
            .map(read)
            .collect::<Result<_, _>>()?,
        next: page.next,
    })
}

pub async fn usage() -> Result<Usage, ApiError> {
    let told: UsageDTO = http::parsed(http::sent(Request::get(USAGE)).await?, ASSET).await?;

    Ok(Usage {
        used: told.used,
        most: told.most,
    })
}

/// Uploads an image as the signed-in account's own, and returns the new asset's id.
pub async fn upload_own(bytes: &[u8]) -> Result<String, ApiError> {
    let uploaded: AssetDTO =
        http::parsed(http::sent_bytes(Request::post(ASSETS), bytes).await?, ASSET).await?;

    Ok(uploaded.id)
}

/// Uploads an image file into a project, and returns the new asset's id.
/// Every member who may read the project may see it.
/// `watched` is told how far the upload got. Given a signal, the upload stops once the signal is aborted.
pub async fn upload_into(
    project: &str,
    file: &File,
    watched: impl Fn(f64) + 'static,
    halted: Option<&AbortSignal>,
) -> Result<String, ApiError> {
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| ApiError::Unexpected)?;
    let into = format!("{ASSETS}?project={}", encoded(project));
    let uploaded: AssetDTO = http::parsed(
        http::posted_watched(&into, &Uint8Array::new(&buffer).to_vec(), watched, halted).await?,
        ASSET,
    )
    .await?;

    Ok(uploaded.id)
}

/// Uploads image files into a project, one after the other, telling how each goes by its place in `files`.
/// A file larger than an upload may be is refused at once, before anything is sent; the others are uploaded in turn.
/// `progressed` is told how far a file got, `finished` the new asset's id or why the file was refused.
/// Given a signal, the uploads stop once it is aborted, and nothing more is told.
pub async fn upload_each_into(
    project: &str,
    files: Vec<File>,
    halted: Option<AbortSignal>,
    progressed: impl Fn(usize, f64) + Clone + 'static,
    finished: impl Fn(usize, Result<String, ApiError>),
) {
    let most = most_each_upload().await.ok().flatten();
    let mut queued = Vec::new();
    for (nth, file) in files.into_iter().enumerate() {
        match most {
            Some(most) if file.size() > most as f64 => finished(nth, Err(too_large(most))),
            _ => queued.push((nth, file)),
        }
    }
    for (nth, file) in queued {
        if halted.as_ref().is_some_and(AbortSignal::aborted) {
            return;
        }
        let progressed = progressed.clone();
        let told = upload_into(
            project,
            &file,
            move |fraction| progressed(nth, fraction),
            halted.as_ref(),
        )
        .await;
        if halted.as_ref().is_some_and(AbortSignal::aborted) {
            return;
        }
        finished(nth, told);
    }
}

/// The same refusal the server gives for an upload past its limit, told before anything is sent.
fn too_large(most: u64) -> ApiError {
    ApiError::Rejected(Refusal {
        kind: Some(refused::LIMIT_REACHED.to_owned()),
        detail: String::new(),
        particulars: vec![
            (
                "bound".to_owned(),
                Particular::Text(bound::UPLOAD.to_owned()),
            ),
            (
                "most".to_owned(),
                Particular::Count(i64::try_from(most).unwrap_or(i64::MAX)),
            ),
        ],
    })
}

/// One asset a project holds, with its media type, such as `image/png`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub id: String,
    pub content_type: String,
    pub uploaded_at: OffsetDateTime,
}

/// One page of a project's assets, the newest first, and the cursor for the next page, if there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPage {
    pub assets: Vec<Asset>,
    pub next: Option<String>,
}

fn assets_listing(project: &str, of_type: Option<AssetType>, after: Option<&str>) -> String {
    addressed(
        &format!("{ASSETS}/projects/{}", encoded(project)),
        &[
            ("type", of_type.map(|of_type| of_type.named().to_owned())),
            ("after", after.map(str::to_owned)),
        ],
    )
}

/// Lists a project's assets, the newest first, a page at a time.
/// Given a type, it lists only the assets of that type.
pub async fn assets_in(
    project: &str,
    of_type: Option<AssetType>,
    after: Option<&str>,
) -> Result<AssetPage, ApiError> {
    let page: ProjectAssetsDTO = http::parsed(
        http::sent(Request::get(&assets_listing(project, of_type, after))).await?,
        ASSET,
    )
    .await?;
    let assets = page
        .assets
        .into_iter()
        .map(|asset| {
            Ok(Asset {
                uploaded_at: OffsetDateTime::parse(&asset.uploaded_at, &Rfc3339)
                    .map_err(|_| ApiError::Unexpected)?,
                content_type: asset.content_type,
                id: asset.id,
            })
        })
        .collect::<Result<_, ApiError>>()?;

    Ok(AssetPage {
        assets,
        next: page.next,
    })
}

/// Copies an image into a project, and returns the copy's id.
/// Given a signal, the copying stops once the signal is aborted.
pub async fn copy_into(
    image: &str,
    project: &str,
    halted: Option<&AbortSignal>,
) -> Result<String, ApiError> {
    let copies = format!("{ASSETS}/{image}/copies?project={}", encoded(project));
    let copy: AssetDTO = http::parsed(
        http::sent(Request::post(&copies).abort_signal(halted)).await?,
        ASSET,
    )
    .await?;

    Ok(copy.id)
}

/// The most a single upload may be, in bytes, or nothing if there is no such limit.
pub async fn most_each_upload() -> Result<Option<u64>, ApiError> {
    let limits: UploadLimitsDTO =
        http::parsed(http::sent(Request::get(LIMITS)).await?, "limits").await?;

    Ok(limits.each_upload)
}

/// What the client is told of one asset: the project that holds it, if one does, and what uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetInfo {
    pub project: Option<String>,
    pub uses: Vec<Use>,
}

impl AssetInfo {
    /// Whether the asset is used for nothing but the given account's picture.
    /// Only then may the account decide to delete it when it shows another picture.
    pub fn only_as_picture_of(&self, account: &str) -> bool {
        matches!(
            self.uses.as_slice(),
            [used] if used.purpose == Purpose::AccountPicture
                && used.by == Holder::Account(account.to_owned())
        )
    }
}

/// Tells what is known of one asset to whoever may see it.
pub async fn asset(id: &str) -> Result<AssetInfo, ApiError> {
    let told: AssetDTO = http::parsed(
        http::sent(Request::get(&format!("{ASSETS}/{id}"))).await?,
        ASSET,
    )
    .await?;

    Ok(info_of(told))
}

fn info_of(told: AssetDTO) -> AssetInfo {
    AssetInfo {
        project: match told.owner {
            OwnerDTO::Project(project) => Some(project),
            OwnerDTO::Account(_) => None,
        },
        uses: told.uses.into_iter().map(use_read).collect(),
    }
}

pub async fn delete(id: &str) -> Result<(), ApiError> {
    http::checked(
        http::sent(Request::delete(&format!("{ASSETS}/{id}"))).await?,
        ASSET,
    )
    .await
    .map(|_| ())
}

/// Returns how much storage the whole instance uses, and its limit. For an operator only.
pub async fn instance() -> Result<Usage, ApiError> {
    let told: InstanceStorageDTO =
        http::parsed(http::sent(Request::get(INSTANCE)).await?, ASSET).await?;

    Ok(Usage {
        used: told.used,
        most: told.most,
    })
}

/// Lists the accounts that use storage, the ones using the most first. For an operator only.
/// With a search term, only accounts whose name or email contains it are listed.
pub async fn accounts(search: &str, after: Option<&str>) -> Result<AccountsPage, ApiError> {
    let page: AccountsStorageDTO = http::parsed(
        http::sent(Request::get(&accounts_listing(search, after))).await?,
        ASSET,
    )
    .await?;

    Ok(AccountsPage {
        accounts: page
            .accounts
            .into_iter()
            .map(|told| AccountStorage {
                account: told.account,
                name: told.name,
                email: told.email,
                used: told.used,
                most: told.most,
                own_limit: told.own_limit,
            })
            .collect(),
        next: page.next,
    })
}

/// Lists the projects that use storage, the ones using the most first, with their owners. For an operator only.
/// With a search term, only projects whose name contains it are listed.
pub async fn projects(search: &str, after: Option<&str>) -> Result<ProjectsPage, ApiError> {
    let page: ProjectsStorageDTO = http::parsed(
        http::sent(Request::get(&projects_listing(search, after))).await?,
        ASSET,
    )
    .await?;

    Ok(ProjectsPage {
        projects: page
            .projects
            .into_iter()
            .map(|told| ProjectStorage {
                project: told.project,
                name: told.name,
                used: told.used,
                most: told.most,
                owners: told
                    .owners
                    .into_iter()
                    .map(|owner| ProjectOwner {
                        account: owner.account,
                        name: owner.name,
                        email: owner.email,
                    })
                    .collect(),
            })
            .collect(),
        next: page.next,
    })
}

/// Gives an account its own storage limit. For an operator only.
pub async fn give_limit(account: &str, most: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent_with(
            Request::put(&limit_of(account)),
            &AccountLimitRequest { most },
        )
        .await?,
        ASSET,
    )
    .await
    .map(|_| ())
}

/// Removes an account's own storage limit, so the default limit applies again. For an operator only.
pub async fn take_back_limit(account: &str) -> Result<(), ApiError> {
    http::checked(
        http::sent(Request::delete(&limit_of(account))).await?,
        ASSET,
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn uploads_are_asked_for_in_their_order_and_from_the_cursor() {
        // expect: the first page in the order asked
        assert_eq!(
            listing(UploadsSorted::Newest, None, None),
            "/api/assets?sort=newest"
        );
        // and: the next page from its cursor
        assert_eq!(
            listing(UploadsSorted::Largest, None, Some("120.5.asset_1")),
            "/api/assets?sort=largest&after=120.5.asset_1"
        );
        // and: only images, when asked for
        assert_eq!(
            listing(
                UploadsSorted::Newest,
                Some(AssetType::Image),
                Some("120.5.asset_1")
            ),
            "/api/assets?sort=newest&type=image&after=120.5.asset_1"
        );
    }

    #[test]
    fn a_project_s_assets_are_asked_for_by_their_type_and_from_the_cursor() {
        // expect: every asset of a project, the first page
        assert_eq!(
            assets_listing("project_1", None, None),
            "/api/assets/projects/project_1"
        );
        // and: only its images, from a cursor
        assert_eq!(
            assets_listing("project_1", Some(AssetType::Image), Some("120.5.asset_1")),
            "/api/assets/projects/project_1?type=image&after=120.5.asset_1"
        );
    }

    #[test]
    fn an_upload_into_a_project_names_it_and_one_s_own_names_none() {
        // given: an upload into a project and one of the account's own
        let told = |owner: OwnerDTO| UploadDTO {
            id: "asset_1".to_owned(),
            owner,
            project_name: None,
            content_type: "image/png".to_owned(),
            width: 64,
            height: 64,
            total_size: 2_048,
            uploaded_at: "2026-11-13T10:00:00Z".to_owned(),
            deletable: true,
            uses: Vec::new(),
        };

        // expect: the project is read from the one, and none from the other
        assert_eq!(
            read(told(OwnerDTO::Project("project_1".to_owned())))
                .expect("a readable upload")
                .project,
            Some("project_1".to_owned())
        );
        assert_eq!(
            read(told(OwnerDTO::Account("account_ada".to_owned())))
                .expect("a readable upload")
                .project,
            None
        );
    }

    #[test]
    fn the_accounts_are_asked_for_the_most_used_first_and_by_their_search() {
        // expect: no search term asks for every account
        assert_eq!(
            accounts_listing("  ", None),
            "/api/assets/storage/accounts?sort=most-used"
        );
        // and: a page goes on from its cursor
        assert_eq!(
            accounts_listing("", Some("2048.account_ada")),
            "/api/assets/storage/accounts?sort=most-used&after=2048.account_ada"
        );
        // and: a term is trimmed and sent as part of the address
        assert_eq!(
            accounts_listing(" ada lovelace ", None),
            "/api/assets/storage/accounts?sort=most-used&search=ada%20lovelace"
        );
        // and: signs that mean something in an address are encoded
        assert_eq!(
            accounts_listing("ada+1@example.com&x", None),
            "/api/assets/storage/accounts?sort=most-used&search=ada%2B1%40example.com%26x"
        );
        // and: letters beyond ascii are encoded byte by byte
        assert_eq!(
            accounts_listing("jürgen", None),
            "/api/assets/storage/accounts?sort=most-used&search=j%C3%BCrgen"
        );
    }

    #[test]
    fn the_projects_are_asked_for_like_the_accounts() {
        // expect: the projects' own address, with the same order, cursor and search
        assert_eq!(
            projects_listing("loom", Some("2048.project_1")),
            "/api/assets/storage/projects?sort=most-used&after=2048.project_1&search=loom"
        );
    }

    #[test]
    fn an_upload_s_uses_are_read_with_what_holds_them_and_a_purpose_unknown_here_is_kept_as_other()
    {
        // given: an upload used as a picture, in a scene, and for something this client does not know
        let told = UploadDTO {
            id: "asset_1".to_owned(),
            owner: OwnerDTO::Account("account_ada".to_owned()),
            project_name: None,
            content_type: "image/png".to_owned(),
            width: 64,
            height: 64,
            total_size: 2_048,
            uploaded_at: "2026-11-13T10:00:00Z".to_owned(),
            deletable: false,
            uses: vec![
                AssetUseDTO {
                    purpose: "account-picture".to_owned(),
                    by: HolderDTO::Account("account_ada".to_owned()),
                },
                AssetUseDTO {
                    purpose: "scene-image".to_owned(),
                    by: HolderDTO::Scene("scene_dawn".to_owned()),
                },
                AssetUseDTO {
                    purpose: "idea-cover".to_owned(),
                    by: HolderDTO::Account("account_ada".to_owned()),
                },
            ],
        };

        // expect: every use is read with what holds it, the unknown one as another purpose
        assert_eq!(
            read(told).expect("a readable upload").uses,
            vec![
                Use {
                    purpose: Purpose::AccountPicture,
                    by: Holder::Account("account_ada".to_owned()),
                },
                Use {
                    purpose: Purpose::SceneImage,
                    by: Holder::Scene(SceneId::from("scene_dawn".to_owned())),
                },
                Use {
                    purpose: Purpose::Other,
                    by: Holder::Account("account_ada".to_owned()),
                },
            ]
        );
    }

    #[test]
    fn an_asset_is_only_a_picture_when_that_is_its_one_use_and_the_account_is_this_one() {
        // given: an asset told with some uses
        let told = |uses: Vec<AssetUseDTO>| AssetDTO {
            id: "asset_1".to_owned(),
            version: 1,
            owner: OwnerDTO::Account("account_ada".to_owned()),
            content_type: "image/jpeg".to_owned(),
            width: 512,
            height: 512,
            widths: vec![96, 192, 384, 512],
            original_size: 40_000,
            total_size: 90_000,
            uses,
        };
        let picture_of = |account: &str| AssetUseDTO {
            purpose: purpose::ACCOUNT_PICTURE.to_owned(),
            by: HolderDTO::Account(account.to_owned()),
        };

        // expect: Ada's picture alone is hers to decide about
        assert!(info_of(told(vec![picture_of("account_ada")])).only_as_picture_of("account_ada"));
        // and: not when it is someone else's, used for more, or used for nothing
        assert!(!info_of(told(vec![picture_of("account_bo")])).only_as_picture_of("account_ada"));
        assert!(
            !info_of(told(vec![
                picture_of("account_ada"),
                AssetUseDTO {
                    purpose: "idea-cover".to_owned(),
                    by: HolderDTO::Account("account_ada".to_owned()),
                },
            ]))
            .only_as_picture_of("account_ada")
        );
        assert!(!info_of(told(vec![])).only_as_picture_of("account_ada"));
    }

    #[test]
    fn a_sort_reads_back_from_its_name() {
        // expect: each sort survives its name
        for sorted in [UploadsSorted::Newest, UploadsSorted::Largest] {
            assert_eq!(UploadsSorted::of(sorted.named()), Some(sorted));
        }
        assert_eq!(UploadsSorted::of("oldest"), None);
    }
}
