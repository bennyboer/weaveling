use gloo_net::http::Request;
use members_contract::MemberDTO;

use crate::http;
use crate::http::ApiError;
use crate::members::listed;
use crate::members::listed::Member;

const MEMBERS: &str = "/api/members";
const MEMBER: &str = "member";

pub async fn members_of(project: &str) -> Result<Vec<Member>, ApiError> {
    let told: Vec<MemberDTO> = http::parsed(
        http::sent(Request::get(&format!("{MEMBERS}?project={project}"))).await?,
        MEMBER,
    )
    .await?;

    told.into_iter().map(listed::listed).collect()
}
