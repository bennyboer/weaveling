use gloo_net::http::Request;
use invitations_contract::{InvitationPageDTO, InvitationSummaryDTO, InviteRequest};

use crate::http;
use crate::http::ApiError;
use crate::invitations::listed;
use crate::invitations::listed::{InvitationPage, ListedInvitation, Scope};

const INVITATIONS: &str = "/api/invitations";
const INVITATION: &str = "invitation";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitedInto {
    Instance,
    Project(String),
}

impl InvitedInto {
    fn listing(&self, after: Option<&str>) -> String {
        let asked: Vec<String> = [
            match self {
                Self::Instance => None,
                Self::Project(project) => Some(format!("project={project}")),
            },
            after.map(|after| format!("after={after}")),
        ]
        .into_iter()
        .flatten()
        .collect();

        if asked.is_empty() {
            INVITATIONS.to_owned()
        } else {
            format!("{INVITATIONS}?{}", asked.join("&"))
        }
    }
}

pub async fn list_invitations(
    into: &InvitedInto,
    after: Option<&str>,
) -> Result<InvitationPage, ApiError> {
    let page: InvitationPageDTO = http::parsed(
        http::sent(Request::get(&into.listing(after))).await?,
        INVITATION,
    )
    .await?;

    InvitationPage::of(page)
}

pub async fn invite(email: &str, scope: &Scope) -> Result<ListedInvitation, ApiError> {
    let payload = InviteRequest {
        email: email.to_owned(),
        scope: scope.told(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instance_s_invitations_are_asked_for_bare_and_a_project_s_by_the_project() {
        assert_eq!(InvitedInto::Instance.listing(None), "/api/invitations");
        assert_eq!(
            InvitedInto::Instance.listing(Some("invitation_9")),
            "/api/invitations?after=invitation_9"
        );
        let loom = InvitedInto::Project("project_1".to_owned());
        assert_eq!(loom.listing(None), "/api/invitations?project=project_1");
        assert_eq!(
            loom.listing(Some("invitation_9")),
            "/api/invitations?project=project_1&after=invitation_9"
        );
    }
}
