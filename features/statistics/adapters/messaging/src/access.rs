use statistics_core::StatisticsCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    StatisticsCore,
    "statistics-remember-roles",
    "A change to a project's members did not reach the statistics, so who is told its word counts is not as it should be."
);

projects_contract::access_trash_listener!(
    RememberTrashListener,
    StatisticsCore,
    "statistics-remember-trash",
    "A project put in the trash or restored did not reach the statistics, so its word counts may be told when they should not, or not when they should."
);
