use std::collections::BTreeMap;
use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::account::Account;
use crate::id::AccountId;
use crate::lock::{Lock, LockKind};
use crate::name::AccountName;
use email::EmailAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: AccountId,
    pub version: Version,
    pub name: AccountName,
    pub email: EmailAddress,
    pub verified: bool,
    pub operator: bool,
    pub signed_up_at: OffsetDateTime,
    pub locks: BTreeMap<LockKind, Lock>,
    pub second_factor: bool,
}

/// What an account is in a project, as the project's members last told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InProject {
    Owner,
    Member,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of accounts could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait AccountCatalog: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AccountSummary,
    ) -> Result<(), CatalogError>;

    async fn listed_after(
        &self,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, CatalogError>;

    /// Forgets a deleted account whole: its place in the list and what it held in any project.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError>;

    /// Remembers what an account is in a project, none where it is gone, as of when, by the members' version, so an older word heard late changes nothing, a join after the removal it preceded among them.
    async fn remember_holding(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        held: Option<InProject>,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Remembers a project's members disbanded, its project deleted, as of when: nobody holds anything in it any more.
    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Forgets, at most so many of each, the projects disbanded before then with everything held in them, and the members gone before then: once no word about them may be retried, nothing is left for them to guard against.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    /// Remembers whether a project is in the trash, by the project's version, so an older word heard late changes nothing.
    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), CatalogError>;

    /// Forgets a deleted project's place in the trash whole, so it holds only what is in the trash.
    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<(), CatalogError>;

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, CatalogError>;

    /// The projects others are in that the account is the last owner of, out of the trash, by project.
    async fn projects_to_hand_over(&self, account: &AccountId)
    -> Result<Vec<String>, CatalogError>;
}

impl AccountSummary {
    pub fn of(id: AccountId, version: Version, account: &Account) -> Self {
        Self {
            id,
            version,
            name: account.name().clone(),
            email: account.email().clone(),
            verified: account.is_verified(),
            operator: account.is_operator(),
            signed_up_at: account.signed_up_at(),
            locks: account.locks().clone(),
            second_factor: account.second_factor().is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{Agent, Aggregate, AggregateContext, AggregateId, EventMetadata};

    use super::*;
    use crate::account::{AccountEvent, KIND};
    use crate::lock::LockReason;

    fn stamped(version: u64, seconds: i64) -> EventMetadata {
        EventMetadata {
            aggregate: AggregateId::from("account_1"),
            kind: KIND,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: clock::at(seconds),
            is_snapshot: false,
            context: AggregateContext::default(),
        }
    }

    #[test]
    fn a_summary_tells_what_the_account_is_at_its_version() {
        // given: a locked operator not yet verified
        let email = EmailAddress::new("ada@example.com").expect("a plain address");
        let name = AccountName::new("Ada").expect("a plain name");
        let mut account = Account::from_first(
            &AccountEvent::SignedUp {
                email: email.clone(),
                name: name.clone(),
            },
            &stamped(1, 1_000),
        )
        .expect("signing up gives birth");
        account.apply(&AccountEvent::BecameOperator, &stamped(2, 2_000));
        let reason = LockReason::new("Sent spam.").expect("a plain reason");
        account.apply(
            &AccountEvent::Locked {
                kind: LockKind::Operator,
                reason: reason.clone(),
            },
            &stamped(3, 3_000),
        );
        let id = AccountId::generate(clock::at(1_000));

        // expect: its summary tells all of that
        assert_eq!(
            AccountSummary::of(id, Version::of(3), &account),
            AccountSummary {
                id,
                version: Version::of(3),
                name,
                email,
                verified: false,
                operator: true,
                signed_up_at: clock::at(1_000),
                locks: BTreeMap::from([(
                    LockKind::Operator,
                    Lock {
                        reason,
                        since: clock::at(3_000),
                    }
                )]),
                second_factor: false,
            }
        );

        // when: its address is verified
        account.apply(&AccountEvent::EmailVerified, &stamped(4, 4_000));

        // then: its summary at the next version tells it verified
        assert!(AccountSummary::of(id, Version::of(4), &account).verified);
    }
}
