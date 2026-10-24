use gloo_net::http::Request;
use invitations_contract::InvitationPageDTO;

use crate::http;
use crate::http::ApiError;
use crate::invitations::listed::InvitationPage;

const INVITATIONS: &str = "/api/invitations";
const INVITATION: &str = "invitation";

pub async fn list_invitations(after: Option<&str>) -> Result<InvitationPage, ApiError> {
    let asked = match after {
        Some(after) => format!("{INVITATIONS}?after={after}"),
        None => INVITATIONS.to_owned(),
    };
    let page: InvitationPageDTO =
        http::parsed(http::sent(Request::get(&asked)).await?, INVITATION).await?;

    InvitationPage::of(page)
}
