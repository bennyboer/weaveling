use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use transacting::Transaction;

use crate::Role;

/// The access updates a feature's core receives: who may do what in its projects.
///
/// The listeners laid out by the members' and the projects' contracts call these methods.
/// This way every feature receives the updates in the same way, and the compiler checks that each core handles them.
#[async_trait]
pub trait HearsAccess: Send + Sync {
    type Refused: Error + Send + Sync + 'static;

    /// An account's role in a project, as the project's members reported it at a version.
    /// No role means the account is no longer a member.
    async fn remember_role(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
    ) -> Result<(), Self::Refused>;

    /// The members of a project were disbanded, because the project was deleted.
    async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), Self::Refused>;

    /// Whether a project is in the trash, as the projects reported it at a version.
    async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), Self::Refused>;
}

/// The updates a feature's core receives about who runs the instance.
///
/// The listener laid out by the accounts' contract calls this, in features that ask for it.
#[async_trait]
pub trait HearsOperators: Send + Sync {
    type Refused: Error + Send + Sync + 'static;

    /// Whether an account runs the instance, as the accounts reported it at a version.
    async fn remember_operator(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        operator: bool,
    ) -> Result<(), Self::Refused>;
}
