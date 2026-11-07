use std::ops::Deref;

use std::collections::BTreeMap;
use transacting::{Transaction, Transactions, Tried};

use accounts_core::{
    AccountCatalog, AccountId, AccountName, AccountSummary, CatalogError, EmailAddress, Lock,
    LockKind, LockReason,
};
use eventsourcing::Version;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: AccountCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        Committing {
            catalog: self.store(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, C> {
    catalog: &'a C,
    transactions: &'a dyn Transactions,
}

fn unreachable(failure: transacting::Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

impl<C: AccountCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn remember(&self, summary: &AccountSummary) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, summary)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }
}

impl<C> Deref for Committing<'_, C> {
    type Target = C;

    fn deref(&self) -> &C {
        self.catalog
    }
}

pub fn a_summary(id: AccountId, email: &str) -> AccountSummary {
    AccountSummary {
        id,
        version: Version::of(1),
        name: AccountName::new("Ada Lovelace").expect("a plain name"),
        email: EmailAddress::new(email).expect("a plain address"),
        verified: false,
        operator: false,
        signed_up_at: clock::at(1_000),
        locks: BTreeMap::new(),
        second_factor: false,
    }
}

fn an_account_signed_up_at(seconds: i64) -> AccountId {
    AccountId::generate(clock::at(seconds))
}

async fn listed(catalog: &Committing<'_, impl AccountCatalog>) -> Vec<AccountSummary> {
    catalog
        .listed_after(None, 100)
        .await
        .expect("listing should succeed")
}

pub async fn a_remembered_account_is_listed_as_it_was(
    catalog: &Committing<'_, impl AccountCatalog>,
) {
    // given: a summary of a locked operator with a second factor
    let summary = AccountSummary {
        version: Version::of(4),
        verified: false,
        operator: true,
        second_factor: true,
        signed_up_at: clock::at(1_700_000_000),
        locks: BTreeMap::from([(
            LockKind::Operator,
            Lock {
                reason: LockReason::new("Sent spam.\nTwice.").expect("a plain reason"),
                since: clock::at(1_700_086_400),
            },
        )]),
        ..a_summary(an_account_signed_up_at(1_000), "ada@example.com")
    };

    // when: it is remembered
    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    // then: it is listed as it was
    assert_eq!(listed(catalog).await, vec![summary]);
}

pub async fn an_empty_catalog_lists_nothing(catalog: &Committing<'_, impl AccountCatalog>) {
    // expect: an empty catalog lists nothing
    assert!(listed(catalog).await.is_empty());
}

pub async fn a_newer_summary_replaces_the_older(catalog: &Committing<'_, impl AccountCatalog>) {
    // given: a remembered summary of an account
    let id = an_account_signed_up_at(1_000);
    catalog
        .remember(&a_summary(id, "ada@example.com"))
        .await
        .expect("remembering should succeed");

    // when: a newer summary of it is remembered
    let newer = AccountSummary {
        version: Version::of(2),
        verified: true,
        ..a_summary(id, "ada@example.com")
    };
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    // then: only the newer is listed
    assert_eq!(listed(catalog).await, vec![newer]);
}

pub async fn an_older_summary_arriving_late_changes_nothing(
    catalog: &Committing<'_, impl AccountCatalog>,
) {
    // given: a remembered summary of an account
    let id = an_account_signed_up_at(1_000);
    let newer = AccountSummary {
        version: Version::of(3),
        verified: true,
        ..a_summary(id, "ada@example.com")
    };
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    // when: an older summary of it is remembered
    catalog
        .remember(&AccountSummary {
            version: Version::of(2),
            ..a_summary(id, "ada@example.com")
        })
        .await
        .expect("remembering should succeed");

    // then: the newer is still listed
    assert_eq!(
        listed(catalog).await,
        vec![newer],
        "messages may come late or twice, and an older account must not overwrite a newer"
    );
}

pub async fn the_newest_account_is_listed_first(catalog: &Committing<'_, impl AccountCatalog>) {
    // given: accounts remembered out of the order they signed up in
    for (seconds, email) in [
        (2_000, "grace@example.com"),
        (1_000, "ada@example.com"),
        (3_000, "hedy@example.com"),
    ] {
        catalog
            .remember(&a_summary(an_account_signed_up_at(seconds), email))
            .await
            .expect("remembering should succeed");
    }

    // when: they are listed
    let order: Vec<String> = listed(catalog)
        .await
        .iter()
        .map(|summary| summary.email.to_string())
        .collect();

    // then: the newest come first
    assert_eq!(
        order,
        ["hedy@example.com", "grace@example.com", "ada@example.com"]
    );
}

pub async fn a_page_holds_at_most_what_was_asked_and_the_next_goes_on_after_it(
    catalog: &Committing<'_, impl AccountCatalog>,
) {
    // given: three remembered accounts
    for seconds in [1_000, 2_000, 3_000] {
        catalog
            .remember(&a_summary(
                an_account_signed_up_at(seconds),
                &format!("someone{seconds}@example.com"),
            ))
            .await
            .expect("remembering should succeed");
    }

    // when: a page of two is listed
    let first = catalog
        .listed_after(None, 2)
        .await
        .expect("listing should succeed");
    // and: the page after it
    let next = catalog
        .listed_after(Some(&first[1].id), 2)
        .await
        .expect("listing should succeed");

    // then: the first holds the two newest
    assert_eq!(
        first
            .iter()
            .map(|summary| summary.email.to_string())
            .collect::<Vec<_>>(),
        ["someone3000@example.com", "someone2000@example.com"]
    );
    // and: the next holds the rest
    assert_eq!(
        next.iter()
            .map(|summary| summary.email.to_string())
            .collect::<Vec<_>>(),
        ["someone1000@example.com"]
    );
}

pub async fn what_was_remembered_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl AccountCatalog>,
) {
    // given: an account remembered in a transaction
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .remember(
            &mut transaction,
            &a_summary(an_account_signed_up_at(1_000), "ada@example.com"),
        )
        .await
        .expect("remembering should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: nothing is listed
    assert!(listed(catalog).await.is_empty());
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::catalog_conformance_case!(
            $workbench,
            what_was_remembered_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::catalog_conformance_case!($workbench, a_remembered_account_is_listed_as_it_was);
        $crate::catalog_conformance_case!($workbench, an_empty_catalog_lists_nothing);
        $crate::catalog_conformance_case!($workbench, a_newer_summary_replaces_the_older);
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_summary_arriving_late_changes_nothing
        );
        $crate::catalog_conformance_case!($workbench, the_newest_account_is_listed_first);
        $crate::catalog_conformance_case!(
            $workbench,
            a_page_holds_at_most_what_was_asked_and_the_next_goes_on_after_it
        );
    };
}
