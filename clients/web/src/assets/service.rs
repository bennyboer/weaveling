use assets_contract::{
    AccountLimitRequest, AccountsStorageDTO, InstanceStorageDTO, OwnerDTO, ProjectsStorageDTO,
    UploadDTO, UploadsDTO, UsageDTO, sort,
};
use gloo_net::http::Request;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http;
use crate::http::ApiError;

const ASSETS: &str = "/api/assets";
const USAGE: &str = "/api/assets/usage";
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

/// One of the signed-in account's uploads.
/// `project` is missing for the account's own images, and `project_name` also when the name is not known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub id: String,
    pub project: Option<String>,
    pub project_name: Option<String>,
    pub total_size: u64,
    pub uploaded_at: OffsetDateTime,
    pub deletable: bool,
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
    let asked: Vec<String> = [
        Some(format!("sort={}", sort::MOST_USED)),
        after.map(|after| format!("after={after}")),
        Some(search.trim())
            .filter(|term| !term.is_empty())
            .map(|term| format!("search={}", encoded(term))),
    ]
    .into_iter()
    .flatten()
    .collect();

    format!("{listing}?{}", asked.join("&"))
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

fn listing(sorted: UploadsSorted, after: Option<&str>) -> String {
    match after {
        Some(after) => format!("{ASSETS}?sort={}&after={after}", sorted.named()),
        None => format!("{ASSETS}?sort={}", sorted.named()),
    }
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
        id: told.id,
    })
}

pub async fn uploads(sorted: UploadsSorted, after: Option<&str>) -> Result<UploadPage, ApiError> {
    let page: UploadsDTO = http::parsed(
        http::sent(Request::get(&listing(sorted, after))).await?,
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
            listing(UploadsSorted::Newest, None),
            "/api/assets?sort=newest"
        );
        // and: the next page from its cursor
        assert_eq!(
            listing(UploadsSorted::Largest, Some("120.5.asset_1")),
            "/api/assets?sort=largest&after=120.5.asset_1"
        );
    }

    #[test]
    fn an_upload_into_a_project_names_it_and_one_s_own_names_none() {
        // given: an upload into a project and one of the account's own
        let told = |owner: OwnerDTO| UploadDTO {
            id: "asset_1".to_owned(),
            owner,
            project_name: None,
            format: "png".to_owned(),
            width: 64,
            height: 64,
            total_size: 2_048,
            uploaded_at: "2026-11-13T10:00:00Z".to_owned(),
            deletable: true,
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
    fn a_sort_reads_back_from_its_name() {
        // expect: each sort survives its name
        for sorted in [UploadsSorted::Newest, UploadsSorted::Largest] {
            assert_eq!(UploadsSorted::of(sorted.named()), Some(sorted));
        }
        assert_eq!(UploadsSorted::of("oldest"), None);
    }
}
