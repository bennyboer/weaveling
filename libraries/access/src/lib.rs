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

/// Who may do what, as far as a feature has heard: each member's role in each project, the projects disbanded and those in the trash, and who runs the instance. Each word comes with the version of what it is about it was told at, the newest heard winning, so one heard late or twice changes nothing; what has ended stays as a tombstone until nothing can be heard late any more.
#[async_trait]
pub trait AccessCatalog: Send + Sync {
    /// Remembers an account's role in a project as of a version of its members, or, with none, that it is a member no more, as of when.
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError>;

    /// Remembers a project's members disbanded, its project deleted: nobody is a member of it any more, whatever is heard of it late.
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

    /// Remembers whether an account runs the instance, as of a version of the account, or that it runs it no more, as of when.
    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError>;

    /// Lets go, at most so many of each, of what ended before then, once nothing about it can be heard late any more: how many.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError>;

    /// The account's role in a project it is a member of, the project not disbanded; whether it is in the trash is asked apart.
    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError>;

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError>;

    /// The projects the account is a member of, neither disbanded nor in the trash.
    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError>;

    /// Whether two accounts are members of one project at least, neither disbanded nor in the trash.
    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError>;

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError>;

    /// The owners of a project not disbanded, by their accounts' ids.
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
