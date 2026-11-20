use outline_core::OutlinesCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    OutlinesCore,
    "outline-remember-roles",
    "A change to a project's members did not reach the outline feature. The wrong people may be able to read or change the project's outline."
);

projects_contract::access_standing_listener!(
    RememberStandingListener,
    OutlinesCore,
    "outline-remember-standing",
    "The outline feature did not learn that a project was started, put in the trash, restored or deleted. Its outline may be shown while it is in the trash, or not shown after it was restored."
);
