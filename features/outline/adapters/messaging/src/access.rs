use outline_core::OutlinesCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    OutlinesCore,
    "outline-remember-roles",
    "A change to a project's members did not reach the outline, so who may read and change the project's outline is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    OutlinesCore,
    "outline-remember-trash",
    "A project put in the trash or restored did not reach the outline, so its outline may be shown when it should not, or not when it should."
);
