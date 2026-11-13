use appearances_core::AppearancesCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    AppearancesCore,
    "appearances-remember-roles",
    "A change to a project's members did not reach the appearances feature. The wrong people may be shown where the project's ideas appear."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    AppearancesCore,
    "appearances-remember-trash",
    "The appearances feature did not learn that a project was put in the trash or restored. Where its ideas appear may be shown while it is in the trash, or not shown after it was restored."
);
