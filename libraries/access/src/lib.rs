mod hearing;
mod kept;
mod memory;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use hearing::{HearsAccess, HearsOperators};
pub use kept::KeptAccess;
pub use memory::InMemoryAccess;

#[cfg(feature = "postgres")]
pub use postgres::PostgresAccess;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteAccess;

use std::error::Error;

use async_trait::async_trait;
use authorizing::Act;
use eventsourcing::{AccountId, Version};
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

#[derive(Debug, Error)]
pub enum AccessError {
    #[error("who may do what could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// What a member may do in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Owner,
    Author,
    Reader,
}

impl Role {
    pub fn permits(self, act: Act) -> bool {
        match self {
            Role::Owner => true,
            Role::Author => matches!(act, Act::Read | Act::Write),
            Role::Reader => act == Act::Read,
        }
    }
}

/// Who may do what, as far as a feature has heard.
///
/// It stores each member's role in each project, which projects are disbanded or in the trash, and which accounts run the instance.
/// Each update comes with the version of the thing it is about.
/// The newest version wins, so an update that arrives late or twice changes nothing.
/// Things that have ended stay as tombstones until no late update can arrive any more.
#[async_trait]
pub trait AccessCatalog: Send + Sync {
    /// Stores an account's role in a project, as of a version of the project's members.
    /// No role means the account is no longer a member, and `at` records since when.
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError>;

    /// Stores that a project's members were disbanded, because the project was deleted.
    /// Nobody is a member of it any more, whatever late updates about it arrive.
    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError>;

    /// Remembers whether a project is in the trash, as of a version of the project.
    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError>;

    /// Stores whether an account runs the instance, as of a version of the account.
    /// When it no longer does, `at` records since when.
    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError>;

    /// Deletes the tombstones of things that ended before `before`.
    /// Call it only once no late update about them can arrive any more.
    /// It deletes at most `at_most` of each kind.
    /// Returns how many were deleted.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError>;

    /// The account's role in a project it is a member of, unless the project is disbanded.
    /// Whether the project is in the trash is checked separately.
    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError>;

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError>;

    /// The projects the account is a member of, neither disbanded nor in the trash.
    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError>;

    /// Whether two accounts are both members of at least one project that is neither disbanded nor in the trash.
    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError>;

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError>;

    /// The account ids of a project's owners, unless the project is disbanded.
    async fn owners_of(&self, project: &str) -> Result<Vec<AccountId>, AccessError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_owner_may_do_everything_an_author_read_and_write_and_a_reader_read() {
        // expect: each role permits its acts and no other
        for act in [Act::Read, Act::Write, Act::Manage, Act::Restore] {
            assert!(Role::Owner.permits(act), "{act:?}");
            assert_eq!(
                Role::Author.permits(act),
                matches!(act, Act::Read | Act::Write),
                "{act:?}"
            );
            assert_eq!(Role::Reader.permits(act), act == Act::Read, "{act:?}");
        }
    }
}
