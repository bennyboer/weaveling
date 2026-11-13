use outline_core::OutlinesCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    OutlinesCore,
    "outline-remember-roles",
    "A change to a project's members did not reach the outline feature. The wrong people may be able to read or change the project's outline."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    OutlinesCore,
    "outline-remember-trash",
    "The outline feature did not learn that a project was put in the trash or restored. Its outline may be shown while it is in the trash, or not shown after it was restored."
);
