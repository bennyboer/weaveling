use projects_core::ProjectsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    ProjectsCore,
    "projects-remember-roles",
    "A change to a project's members did not reach the projects feature. The wrong people may be able to see, change, trash or restore the project."
);

projects_contract::access_standing_listener!(
    RememberStandingListener,
    ProjectsCore,
    "projects-remember-standing",
    "The projects feature did not update who may do what in a project after it was started, put in the trash, restored or deleted. The project may be read or changed while it is in the trash, or not after it was restored."
);
