use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::asset::ProjectLink;
use crate::catalog::CatalogError;

/// An account's membership of a project, as the assets feature hears of it from the members feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub project: ProjectLink,
    pub account: AccountId,
}

/// Who is a member of which project, kept by the assets feature itself, so who may see an account's own assets is asked of it alone. Each word about a membership comes with the version of the project's members it was told at, the newest heard winning, so one heard late or twice changes nothing.
#[async_trait]
pub trait MembershipCatalog: Send + Sync {
    /// Remembers a membership as of a version of the project's members.
    async fn remember(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
        version: Version,
    ) -> Result<(), CatalogError>;

    /// Forgets a membership as of a version, as when its account left or was removed, keeping what guards against an older word heard late.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Forgets every membership of a project disbanded, its project deleted: nobody is a member of it any more, whatever is heard of it late.
    async fn forget_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Whether two accounts are members of one project at least.
    async fn share_a_project(
        &self,
        transaction: &mut Transaction,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, CatalogError>;

    /// Lets go, at most so many of each, of the projects disbanded before then and the memberships forgotten before then, once no word about them may be heard late any more.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;
}
