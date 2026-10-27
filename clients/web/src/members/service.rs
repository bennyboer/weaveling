use gloo_net::http::Request;
use members_contract::{ChangeRoleRequest, MemberDTO};

use crate::http;
use crate::http::ApiError;
use crate::members::listed;
use crate::members::listed::{Member, Role};

const MEMBERS: &str = "/api/members";
const MEMBER: &str = "member";

fn membership(project: &str, account: &str) -> String {
    format!("{MEMBERS}/{project}/accounts/{account}")
}

fn read(told: Vec<MemberDTO>) -> Result<Vec<Member>, ApiError> {
    told.into_iter().map(listed::listed).collect()
}

pub async fn members_of(project: &str) -> Result<Vec<Member>, ApiError> {
    read(
        http::parsed(
            http::sent(Request::get(&format!("{MEMBERS}?project={project}"))).await?,
            MEMBER,
        )
        .await?,
    )
}

pub async fn change_role(project: &str, account: &str, to: Role) -> Result<Vec<Member>, ApiError> {
    let payload = ChangeRoleRequest {
        role: to.named().to_owned(),
    };

    read(
        http::parsed(
            http::sent_with(Request::patch(&membership(project, account)), &payload).await?,
            MEMBER,
        )
        .await?,
    )
}

pub async fn remove(project: &str, account: &str) -> Result<(), ApiError> {
    http::checked(
        http::sent(Request::delete(&membership(project, account))).await?,
        MEMBER,
    )
    .await
    .map(|_| ())
}
