use ideas_core::IdeasCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    IdeasCore,
    "ideas-remember-roles",
    "A change to a project's members did not reach the ideas feature. The wrong people may be able to see, capture or change the project's ideas."
);

projects_contract::access_standing_listener!(
    RememberStandingListener,
    IdeasCore,
    "ideas-remember-standing",
    "The ideas feature did not learn that a project was started, put in the trash, restored or deleted. Its ideas may be shown or changed while it is in the trash, or not after it was restored."
);
