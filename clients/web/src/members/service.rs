use gloo_net::http::{Request, Response};
use members_contract::{ChangeRoleRequest, MemberDTO, MembershipDTO};

use crate::http;
use crate::http::ApiError;
use crate::members::listed;
use crate::members::listed::{Member, Members, Role};

const MEMBERS: &str = "/api/members";
const MEMBER: &str = "member";
const ETAG: &str = "ETag";

fn membership(project: &str, account: &str) -> String {
    format!("{MEMBERS}/{project}/accounts/{account}")
}

fn read(told: Vec<MemberDTO>) -> Result<Vec<Member>, ApiError> {
    told.into_iter().map(listed::listed).collect()
}

fn version_in(tag: Option<String>) -> Option<u64> {
    tag?.trim().trim_matches('"').parse().ok()
}

async fn to_members(response: Response) -> Result<Members, ApiError> {
    let version = version_in(response.headers().get(ETAG));
    let people = read(http::parsed(response, MEMBER).await?)?;
    let version = version.ok_or(ApiError::Unexpected)?;

    Ok(Members { version, people })
}

pub async fn members_of(project: &str) -> Result<Members, ApiError> {
    to_members(http::sent(Request::get(&format!("{MEMBERS}?project={project}"))).await?).await
}

pub async fn mine() -> Result<Vec<(String, Role)>, ApiError> {
    let told: Vec<MembershipDTO> = http::parsed(
        http::sent(Request::get(&format!("{MEMBERS}/mine"))).await?,
        MEMBER,
    )
    .await?;

    Ok(told
        .into_iter()
        .filter_map(|membership| Some((membership.project, Role::of(&membership.role)?)))
        .collect())
}

pub async fn change_role(
    project: &str,
    account: &str,
    to: Role,
    on: u64,
) -> Result<Members, ApiError> {
    let payload = ChangeRoleRequest {
        role: to.named().to_owned(),
    };

    to_members(
        http::sent_with(
            http::made_on(Request::patch(&membership(project, account)), on),
            &payload,
        )
        .await?,
    )
    .await
}

pub async fn remove(project: &str, account: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(
            Request::delete(&membership(project, account)),
            on,
        ))
        .await?,
        MEMBER,
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_read_as_the_version_it_quotes() {
        // expect: a quoted version is read, with or without quotes and spaces
        assert_eq!(version_in(Some("\"7\"".to_owned())), Some(7));
        assert_eq!(version_in(Some(" 7 ".to_owned())), Some(7));
    }

    #[test]
    fn no_tag_or_one_that_is_no_version_tells_no_version() {
        // expect: neither tells a version
        assert_eq!(version_in(None), None);
        assert_eq!(version_in(Some("\"seven\"".to_owned())), None);
        assert_eq!(version_in(Some("W/\"7\"".to_owned())), None);
    }
}
