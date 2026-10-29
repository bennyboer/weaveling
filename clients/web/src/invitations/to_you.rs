use invitations_contract::InvitationToYouDTO;

use crate::http::ApiError;
use crate::members::listed::Role;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationToYou {
    pub id: String,
    pub project_name: Option<String>,
    pub role: Role,
    pub invited_by: Option<String>,
}

pub fn read(told: Vec<InvitationToYouDTO>) -> Result<Vec<InvitationToYou>, ApiError> {
    told.into_iter()
        .map(|invitation| {
            Ok(InvitationToYou {
                role: Role::of(&invitation.role).ok_or(ApiError::Unexpected)?,
                id: invitation.id,
                project_name: invitation.project_name,
                invited_by: invitation.invited_by,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn told(
        role: &str,
        project_name: Option<&str>,
        invited_by: Option<&str>,
    ) -> InvitationToYouDTO {
        InvitationToYouDTO {
            id: "invitation_1".to_owned(),
            project: "project_1".to_owned(),
            project_name: project_name.map(str::to_owned),
            role: role.to_owned(),
            invited_by: invited_by.map(str::to_owned),
            until: "2026-11-05T08:00:00Z".to_owned(),
        }
    }

    #[test]
    fn an_invitation_to_you_is_read_with_its_project_s_name_its_role_and_who_invited() {
        assert_eq!(
            read(vec![
                told("author", Some("Shared"), Some("Ada")),
                told("reader", None, None),
            ]),
            Ok(vec![
                InvitationToYou {
                    id: "invitation_1".to_owned(),
                    project_name: Some("Shared".to_owned()),
                    role: Role::Author,
                    invited_by: Some("Ada".to_owned()),
                },
                InvitationToYou {
                    id: "invitation_1".to_owned(),
                    project_name: None,
                    role: Role::Reader,
                    invited_by: None,
                },
            ])
        );
    }

    #[test]
    fn a_role_nobody_knows_is_unexpected() {
        assert_eq!(
            read(vec![told("gardener", Some("Shared"), Some("Ada"))]),
            Err(ApiError::Unexpected)
        );
    }
}
