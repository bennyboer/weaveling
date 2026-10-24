use gloo_net::http::Request;
use invitations_contract::{InvitationPageDTO, InvitationSummaryDTO, InviteRequest};

use crate::http;
use crate::http::ApiError;
use crate::invitations::listed;
use crate::invitations::listed::{InvitationPage, ListedInvitation};

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

pub async fn invite(email: &str) -> Result<ListedInvitation, ApiError> {
    let payload = InviteRequest {
        email: email.to_owned(),
    };
    let invited: InvitationSummaryDTO = http::parsed(
        http::sent_with(Request::post(INVITATIONS), &payload).await?,
        INVITATION,
    )
    .await?;

    listed::listed(invited)
}

pub async fn withdraw(id: &str) -> Result<(), ApiError> {
    http::checked(
        http::sent(Request::delete(&format!("{INVITATIONS}/{id}"))).await?,
        INVITATION,
    )
    .await
    .map(|_| ())
}
