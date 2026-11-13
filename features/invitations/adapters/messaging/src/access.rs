use invitations_core::InvitationsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    InvitationsCore,
    "invitations-remember-roles",
    "A change to a project's members did not reach the invitations feature. The wrong people may be able to invite others into the project or see its invitations."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    InvitationsCore,
    "invitations-remember-trash",
    "The invitations feature did not learn that a project was put in the trash or restored. People may invite others into it while it is in the trash, or not be able to after it was restored."
);
