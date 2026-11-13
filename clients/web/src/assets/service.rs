use assets_contract::{OwnerDTO, UploadDTO, UploadsDTO, UsageDTO, sort};
use gloo_net::http::Request;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http;
use crate::http::ApiError;

const ASSETS: &str = "/api/assets";
const USAGE: &str = "/api/assets/usage";
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
    fn a_sort_reads_back_from_its_name() {
        // expect: each sort survives its name
        for sorted in [UploadsSorted::Newest, UploadsSorted::Largest] {
            assert_eq!(UploadsSorted::of(sorted.named()), Some(sorted));
        }
        assert_eq!(UploadsSorted::of("oldest"), None);
    }
}
