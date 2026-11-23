use ::access::{HearsAccess, Role};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use transacting::Transaction;

use crate::error::FeedbackError;
use crate::feedback::FeedbackCore;

#[async_trait]
impl HearsAccess for FeedbackCore {
    type Refused = FeedbackError;

    /// Stores an account's role in a project, or that the account is no longer a member.
    /// The members feature reports this at a version.
    /// The stored roles decide who may read, work through and delete feedback.
    async fn remember_role(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
    ) -> Result<(), FeedbackError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_role(
                    transaction,
                    project,
                    account,
                    version,
                    role,
                    self.clock.now(),
                )
                .await?;

            Ok::<_, FeedbackError>(())
        })
    }

    /// Stores that a project's members were disbanded because the project was deleted.
    /// After this, nobody is a member of the project.
    async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), FeedbackError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_disbanded(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, FeedbackError>(())
        })
    }

    /// Stores whether a project is in the trash, as the projects feature reported it at a version.
    async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), FeedbackError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_trash(transaction, project, version, in_the_trash)
                .await?;

            Ok::<_, FeedbackError>(())
        })
    }
}
