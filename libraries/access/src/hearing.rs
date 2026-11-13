use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use transacting::Transaction;

use crate::Role;

/// What a feature's core is told of who may do what in its projects, by the listeners the members' and the projects' contracts lay out for it: so every feature hears alike, and the compiler holds each core to it.
#[async_trait]
pub trait HearsAccess: Send + Sync {
    type Refused: Error + Send + Sync + 'static;

    /// An account's role in a project, or with none that it is a member no more, as the project's members told it at a version.
    async fn remember_role(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
    ) -> Result<(), Self::Refused>;

    /// A project's members disbanded, its project deleted.
    async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), Self::Refused>;

    /// Whether a project is in the trash, as the projects told it at a version.
    async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), Self::Refused>;
}

/// What a feature's core is told of who runs the instance, by the listener the accounts' contract lays out for it, where the feature asks it.
#[async_trait]
pub trait HearsOperators: Send + Sync {
    type Refused: Error + Send + Sync + 'static;

    /// Whether an account runs the instance, as the accounts told it at a version.
    async fn remember_operator(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        operator: bool,
    ) -> Result<(), Self::Refused>;
}
