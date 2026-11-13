use assets_core::AssetsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    AssetsCore,
    "assets-remember-roles",
    "A change to a project's members did not reach the assets, so who may see and upload its images, or an account's own, is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    AssetsCore,
    "assets-remember-trash",
    "A project put in the trash or restored did not reach the assets, so its images may be shown when they should not, or not when they should."
);

accounts_contract::access_operators_listener!(
    RememberOperatorsListener,
    AssetsCore,
    "assets-remember-operators",
    "A change to who runs the instance did not reach the assets, so storage limits may be set by one who may not, or not by one who may."
);
