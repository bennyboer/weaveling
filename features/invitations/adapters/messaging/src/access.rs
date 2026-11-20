use invitations_core::InvitationsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    InvitationsCore,
    "invitations-remember-roles",
    "A change to a project's members did not reach the invitations feature. The wrong people may be able to invite others into the project or see its invitations."
);

projects_contract::access_standing_listener!(
    RememberStandingListener,
    InvitationsCore,
    "invitations-remember-standing",
    "The invitations feature did not learn that a project was started, put in the trash, restored or deleted. People may invite others into it while it is in the trash, or not be able to after it was restored."
);
