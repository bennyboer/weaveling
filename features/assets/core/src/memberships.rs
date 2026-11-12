use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::Transaction;

use crate::asset::ProjectLink;
use crate::catalog::CatalogError;

/// An account's membership of a project, as the assets feature hears of it from the members feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub project: ProjectLink,
    pub account: AccountId,
}

/// Who is a member of which project, kept by the assets feature itself, so who may see an account's own assets is asked of it alone.
#[async_trait]
pub trait MembershipCatalog: Send + Sync {
    /// Remembers a membership; one remembered already is no failure.
    async fn remember(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
    ) -> Result<(), CatalogError>;

    /// Forgets a membership, as when its account left or was removed; one never remembered is no failure.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
    ) -> Result<(), CatalogError>;

    /// Forgets every membership of a project, as when it is gone.
    async fn forget_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;

    /// Whether two accounts are members of one project at least.
    async fn share_a_project(
        &self,
        transaction: &mut Transaction,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, CatalogError>;
}
