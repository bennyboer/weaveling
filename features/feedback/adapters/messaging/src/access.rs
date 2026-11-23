use feedback_core::FeedbackCore;

members_contract::access_roles_listener!(
    RememberRolesListener,
    FeedbackCore,
    "feedback-remember-roles",
    "A change to a project's members did not reach the feedback feature. The wrong people may see the project's feedback, mark it done or delete it."
);

projects_contract::access_standing_listener!(
    RememberStandingListener,
    FeedbackCore,
    "feedback-remember-standing",
    "The feedback feature did not learn that a project was started, put in the trash, restored or deleted. Its feedback may be shown while it is in the trash, or not shown after it was restored."
);
