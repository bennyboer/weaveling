use ::access::{HearsAccess, Role};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::error::ReadingsError;
use crate::readings::ReadingsCore;

impl ReadingsCore {
    /// Forgets, in batches, the records of accounts that left a project long enough ago
    /// that no message about them can still arrive.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, ReadingsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            Ok(self
                .access
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?)
        })
    }
}

#[async_trait]
impl HearsAccess for ReadingsCore {
    type Refused = ReadingsError;

    /// Stores an account's role in a project, or that the account is no longer a member.
    /// The members feature reports this at a version.
    /// The stored roles decide who may read the book.
    async fn remember_role(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
    ) -> Result<(), ReadingsError> {
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

            Ok::<_, ReadingsError>(())
        })
    }

    /// Stores that a project's members were disbanded because the project was deleted.
    /// After this, nobody is a member of the project.
    async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), ReadingsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_disbanded(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, ReadingsError>(())
        })
    }

    /// Stores whether a project is in the trash, as the projects feature reported it at a version.
    async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), ReadingsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_trash(transaction, project, version, in_the_trash)
                .await?;

            Ok::<_, ReadingsError>(())
        })
    }
}
