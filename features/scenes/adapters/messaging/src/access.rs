use scenes_core::ScenesCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    ScenesCore,
    "scenes-remember-roles",
    "A change to a project's members did not reach the scenes feature. The wrong people may be able to read or write the project's scenes, or edit them together live."
);

projects_contract::access_standing_listener!(
    RememberStandingListener,
    ScenesCore,
    "scenes-remember-standing",
    "The scenes feature did not learn that a project was started, put in the trash, restored or deleted. Its scenes may be shown while it is in the trash, or not shown after it was restored."
);
