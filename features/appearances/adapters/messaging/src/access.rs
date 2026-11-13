use appearances_core::AppearancesCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    AppearancesCore,
    "appearances-remember-roles",
    "A change to a project's members did not reach the appearances, so who is told where its ideas appear is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    AppearancesCore,
    "appearances-remember-trash",
    "A project put in the trash or restored did not reach the appearances, so where its ideas appear may be told when it should not, or not when it should."
);
