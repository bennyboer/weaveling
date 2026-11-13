use invitations_core::InvitationsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    InvitationsCore,
    "invitations-remember-roles",
    "A change to a project's members did not reach the invitations, so who may invite into the project and see its invitations is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    InvitationsCore,
    "invitations-remember-trash",
    "A project put in the trash or restored did not reach the invitations, so one may invite into it in the trash, or not once restored."
);
