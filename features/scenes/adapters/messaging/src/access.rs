use scenes_core::ScenesCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    ScenesCore,
    "scenes-remember-roles",
    "A change to a project's members did not reach the scenes, so who may read and write the project's scenes, or edit them together live, is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    ScenesCore,
    "scenes-remember-trash",
    "A project put in the trash or restored did not reach the scenes, so its scenes may be shown when they should not, or not when they should."
);
