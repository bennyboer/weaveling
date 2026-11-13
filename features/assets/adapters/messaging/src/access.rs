use assets_core::AssetsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    AssetsCore,
    "assets-remember-roles",
    "A change to a project's members did not reach the assets feature. The wrong people may be able to see or upload the project's images. The same goes for seeing an account's own images."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    AssetsCore,
    "assets-remember-trash",
    "The assets feature did not learn that a project was put in the trash or restored. Its images may be shown while it is in the trash, or not shown after it was restored."
);

accounts_contract::access_operators_listener!(
    RememberOperatorsListener,
    AssetsCore,
    "assets-remember-operators",
    "A change to who is an operator of this Weaveling instance did not reach the assets feature. Someone who is no longer an operator may still set storage limits. Or a new operator may not be able to set them."
);
