use boards_core::BoardsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    BoardsCore,
    "boards-remember-roles",
    "A change to a project's members did not reach the boards feature. The wrong people may be able to see the project's boards or pin ideas to them."
);

projects_contract::access_standing_listener!(
    RememberStandingListener,
    BoardsCore,
    "boards-remember-standing",
    "The boards feature did not learn that a project was started, put in the trash, restored or deleted. Its boards may be shown or changed while it is in the trash, or not after it was restored."
);
