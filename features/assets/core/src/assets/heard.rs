use std::collections::BTreeSet;

use access::{HearsAccess, HearsOperators, Role};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::asset::ProjectLink;
use crate::assets::AssetsCore;
use crate::catalog::Person;
use crate::error::AssetsError;
use crate::uses::{Holder, Purpose};

impl AssetsCore {
    /// Stores an account's name and email, as the accounts feature reported them.
    /// Operators see them in the storage overview and can search by them.
    pub async fn remember_person(
        &self,
        within: Option<&mut Transaction>,
        person: &Person,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_person(transaction, person, version)
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Removes the name and email of a deleted account, and ends the use of its picture.
    pub async fn forget_person(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget_person(transaction, account, version, self.clock.now())
                .await?;
            self.uses
                .used_for(
                    transaction,
                    Purpose::AccountPicture,
                    &Holder::Account(account.clone()),
                    &BTreeSet::new(),
                    version,
                    self.clock.now(),
                )
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Stores a project's name, as the projects feature reported it.
    /// Operators see it in the storage overview and can search by it.
    pub async fn remember_project_name(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        version: Version,
        name: &str,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_project_name(transaction, project, version, name)
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Removes the name of a deleted project.
    pub async fn forget_project_name(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget_project_name(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Deletes old tombstones, from the access tables, the stored names and the uses of assets.
    /// A tombstone keeps a late, outdated message from bringing deleted data back.
    /// Once it is old enough, no such message can arrive any more.
    /// Returns how many tombstones were deleted.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AssetsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let access = self
                .access
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let names = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let uses = self
                .uses
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok::<_, AssetsError>(access + names + uses)
        })
    }
}

#[async_trait]
impl HearsAccess for AssetsCore {
    type Refused = AssetsError;

    /// Stores an account's role in a project, or that the account is no longer a member.
    /// The members feature reports this at a version.
    /// The stored roles decide who may see what.
    async fn remember_role(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
    ) -> Result<(), AssetsError> {
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

            Ok::<_, AssetsError>(())
        })
    }

    /// Stores that a project's members were disbanded because the project was deleted.
    /// After this, nobody is a member of the project.
    async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_disbanded(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, AssetsError>(())
        })
    }

    /// Stores whether a project is in the trash, as the projects feature reported it at a version.
    async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_trash(transaction, project, version, in_the_trash)
                .await?;

            Ok::<_, AssetsError>(())
        })
    }
}

#[async_trait]
impl HearsOperators for AssetsCore {
    type Refused = AssetsError;

    /// Stores whether an account runs the instance, as the accounts feature reported it at a version.
    async fn remember_operator(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        operator: bool,
    ) -> Result<(), AssetsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_operator(transaction, account, version, operator, self.clock.now())
                .await?;

            Ok::<_, AssetsError>(())
        })
    }
}
