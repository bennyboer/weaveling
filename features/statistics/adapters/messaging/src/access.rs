use statistics_core::StatisticsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    StatisticsCore,
    "statistics-remember-roles",
    "A change to a project's members did not reach the statistics feature. The wrong people may be shown the project's word counts."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    StatisticsCore,
    "statistics-remember-trash",
    "The statistics feature did not learn that a project was put in the trash or restored. Its word counts may be shown while it is in the trash, or not shown after it was restored."
);
