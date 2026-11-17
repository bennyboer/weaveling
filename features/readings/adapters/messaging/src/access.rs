use readings_core::ReadingsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    ReadingsCore,
    "readings-remember-roles",
    "A change to a project's members did not reach the readings feature. The wrong people may be shown the project's book."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    ReadingsCore,
    "readings-remember-trash",
    "The readings feature did not learn that a project was put in the trash or restored. Its book may be shown while it is in the trash, or not shown after it was restored."
);
