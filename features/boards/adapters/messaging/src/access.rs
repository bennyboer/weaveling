use boards_core::BoardsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    BoardsCore,
    "boards-remember-roles",
    "A change to a project's members did not reach the boards feature. The wrong people may be able to see the project's boards or pin ideas to them."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    BoardsCore,
    "boards-remember-trash",
    "The boards feature did not learn that a project was put in the trash or restored. Its boards may be shown or changed while it is in the trash, or not after it was restored."
);
