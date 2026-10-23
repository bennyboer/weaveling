use std::collections::BTreeMap;

use accounts_core::{
    AccountCatalog, AccountId, AccountName, AccountSummary, EmailAddress, Lock, LockKind,
    LockReason,
};
use eventsourcing::Version;
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: AccountCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: AccountId, email: &str) -> AccountSummary {
    AccountSummary {
        id,
        version: Version::of(1),
        name: AccountName::new("Ada Lovelace").expect("a plain name"),
        email: EmailAddress::new(email).expect("a plain address"),
        verified: false,
        operator: false,
        signed_up_at: at(1_000),
        locks: BTreeMap::new(),
    }
}

fn signed_up(seconds: i64) -> AccountId {
    AccountId::generate(at(seconds))
}

async fn listed(catalog: &impl AccountCatalog) -> Vec<AccountSummary> {
    catalog
        .listed_after(None, 100)
        .await
        .expect("listing should succeed")
}

pub async fn a_remembered_account_is_listed_as_it_was(catalog: &impl AccountCatalog) {
    let summary = AccountSummary {
        version: Version::of(4),
        verified: false,
        operator: true,
        signed_up_at: at(1_700_000_000),
        locks: BTreeMap::from([(
            LockKind::Operator,
            Lock {
                reason: LockReason::new("Sent spam.\nTwice.").expect("a plain reason"),
                since: at(1_700_086_400),
            },
        )]),
        ..a_summary(signed_up(1_000), "ada@example.com")
    };

    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    assert_eq!(listed(catalog).await, vec![summary]);
}

pub async fn an_empty_catalog_lists_nothing(catalog: &impl AccountCatalog) {
    assert!(listed(catalog).await.is_empty());
}

pub async fn a_newer_summary_replaces_the_older(catalog: &impl AccountCatalog) {
    let id = signed_up(1_000);
    catalog
        .remember(&a_summary(id, "ada@example.com"))
        .await
        .expect("remembering should succeed");

    let newer = AccountSummary {
        version: Version::of(2),
        verified: true,
        ..a_summary(id, "ada@example.com")
    };
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    assert_eq!(listed(catalog).await, vec![newer]);
}

pub async fn an_older_summary_arriving_late_changes_nothing(catalog: &impl AccountCatalog) {
    let id = signed_up(1_000);
    let newer = AccountSummary {
        version: Version::of(3),
        verified: true,
        ..a_summary(id, "ada@example.com")
    };
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    catalog
        .remember(&AccountSummary {
            version: Version::of(2),
            ..a_summary(id, "ada@example.com")
        })
        .await
        .expect("remembering should succeed");

    assert_eq!(
        listed(catalog).await,
        vec![newer],
        "messages may come late or twice, and an older account must not overwrite a newer"
    );
}

pub async fn the_newest_account_is_listed_first(catalog: &impl AccountCatalog) {
    for (seconds, email) in [
        (2_000, "grace@example.com"),
        (1_000, "ada@example.com"),
        (3_000, "hedy@example.com"),
    ] {
        catalog
            .remember(&a_summary(signed_up(seconds), email))
            .await
            .expect("remembering should succeed");
    }

    let order: Vec<String> = listed(catalog)
        .await
        .iter()
        .map(|summary| summary.email.to_string())
        .collect();

    assert_eq!(
        order,
        ["hedy@example.com", "grace@example.com", "ada@example.com"]
    );
}

pub async fn a_page_holds_at_most_what_was_asked_and_the_next_goes_on_after_it(
    catalog: &impl AccountCatalog,
) {
    for seconds in [1_000, 2_000, 3_000] {
        catalog
            .remember(&a_summary(
                signed_up(seconds),
                &format!("someone{seconds}@example.com"),
            ))
            .await
            .expect("remembering should succeed");
    }

    let first = catalog
        .listed_after(None, 2)
        .await
        .expect("listing should succeed");
    let next = catalog
        .listed_after(Some(&first[1].id), 2)
        .await
        .expect("listing should succeed");

    assert_eq!(
        first
            .iter()
            .map(|summary| summary.email.to_string())
            .collect::<Vec<_>>(),
        ["someone3000@example.com", "someone2000@example.com"]
    );
    assert_eq!(
        next.iter()
            .map(|summary| summary.email.to_string())
            .collect::<Vec<_>>(),
        ["someone1000@example.com"]
    );
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
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
