use projects_core::ProjectsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    ProjectsCore,
    "projects-remember-roles",
    "A change to a project's members did not reach the projects feature. The wrong people may be able to see, change, trash or restore the project."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    ProjectsCore,
    "projects-remember-trash",
    "The projects feature did not update who may do what in a project after it was put in the trash or restored. The project may be read or changed while it is in the trash, or not after it was restored."
);
