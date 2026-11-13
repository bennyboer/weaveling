use boards_core::BoardsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    BoardsCore,
    "boards-remember-roles",
    "A change to a project's members did not reach the boards, so who may see its boards and pin to them is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    BoardsCore,
    "boards-remember-trash",
    "A project put in the trash or restored did not reach the boards, so its boards may be shown or changed when they should not, or not when they should."
);
