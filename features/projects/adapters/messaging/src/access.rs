use projects_core::ProjectsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    ProjectsCore,
    "projects-remember-roles",
    "A change to a project's members did not reach the projects, so who may see, change, trash or restore the project is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    ProjectsCore,
    "projects-remember-trash",
    "A project put in the trash or restored did not reach who may do what in it, so it may be read or changed in the trash, or not once restored."
);
