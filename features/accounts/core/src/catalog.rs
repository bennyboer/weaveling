use std::collections::BTreeMap;
use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;
use time::OffsetDateTime;

use crate::account::Account;
use crate::email::EmailAddress;
use crate::id::AccountId;
use crate::lock::{Lock, LockKind};
use crate::name::AccountName;

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
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of accounts could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait AccountCatalog: Send + Sync {
    async fn remember(&self, summary: &AccountSummary) -> Result<(), CatalogError>;

    async fn listed_after(
        &self,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, CatalogError>;
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
        }
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{Agent, Aggregate, AggregateId, EventMetadata};
    use time::Duration;

    use super::*;
    use crate::account::{AccountEvent, KIND};
    use crate::lock::LockReason;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn stamped(version: u64, seconds: i64) -> EventMetadata {
        EventMetadata {
            aggregate: AggregateId::from("account_1"),
            kind: KIND,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: at(seconds),
            is_snapshot: false,
        }
    }

    #[test]
    fn a_summary_tells_what_the_account_is_at_its_version() {
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
        let id = AccountId::generate(at(1_000));

        assert_eq!(
            AccountSummary::of(id, Version::of(3), &account),
            AccountSummary {
                id,
                version: Version::of(3),
                name,
                email,
                verified: false,
                operator: true,
                signed_up_at: at(1_000),
                locks: BTreeMap::from([(
                    LockKind::Operator,
                    Lock {
                        reason,
                        since: at(3_000),
                    }
                )]),
            }
        );

        account.apply(&AccountEvent::EmailVerified, &stamped(4, 4_000));
        assert!(AccountSummary::of(id, Version::of(4), &account).verified);
    }
}
