use ideas_core::IdeasCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    IdeasCore,
    "ideas-remember-roles",
    "A change to a project's members did not reach the ideas, so who may see, capture and change its ideas is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    IdeasCore,
    "ideas-remember-trash",
    "A project put in the trash or restored did not reach the ideas, so its ideas may be shown or changed when they should not, or not when they should."
);
