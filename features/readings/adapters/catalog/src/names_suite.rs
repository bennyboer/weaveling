use std::collections::HashMap;

use eventsourcing::AccountId;
use readings_core::{AccountNameCatalog, CatalogError};
use time::OffsetDateTime;
use transacting::Tried;

use crate::suite::Committing;

impl<C: AccountNameCatalog> Committing<'_, C> {
    pub async fn remember_name(&self, account: &AccountId, name: &str) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_name(transaction, account, name)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }

    pub async fn forget_name(
        &self,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget_name(transaction, account, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }

    pub async fn forget_name_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(forgotten)
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }
}

fn an_account(named: &str) -> AccountId {
    AccountId::from(named)
}

fn at(seconds: i64) -> OffsetDateTime {
    clock::at(seconds)
}

async fn named(catalog: &Committing<'_, impl AccountNameCatalog>, account: &str, name: &str) {
    catalog
        .remember_name(&an_account(account), name)
        .await
        .expect("remembering should succeed");
}

async fn gone(
    catalog: &Committing<'_, impl AccountNameCatalog>,
    account: &str,
    at: OffsetDateTime,
) {
    catalog
        .forget_name(&an_account(account), at)
        .await
        .expect("forgetting should succeed");
}

async fn names_of(
    catalog: &Committing<'_, impl AccountNameCatalog>,
    accounts: &[&str],
) -> HashMap<AccountId, String> {
    let asked: Vec<AccountId> = accounts.iter().map(|account| an_account(account)).collect();

    catalog
        .names_of(&asked)
        .await
        .expect("asking should succeed")
}

fn names(pairs: &[(&str, &str)]) -> HashMap<AccountId, String> {
    pairs
        .iter()
        .map(|(account, name)| (an_account(account), (*name).to_owned()))
        .collect()
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl AccountNameCatalog>,
) {
    // given: a name kept
    named(catalog, "account_hedy", "Hedy").await;

    // when: in a transaction that is dropped, another name is kept and the first forgotten
    {
        let mut transaction = catalog.begun().await;
        catalog
            .uncommitted()
            .remember_name(&mut transaction, &an_account("account_ada"), "Ada")
            .await
            .expect("remembering should succeed");
        catalog
            .uncommitted()
            .forget_name(&mut transaction, &an_account("account_hedy"), at(10))
            .await
            .expect("forgetting should succeed");
    }

    // then: only what was there before stands
    assert_eq!(
        names_of(catalog, &["account_hedy", "account_ada"]).await,
        names(&[("account_hedy", "Hedy")])
    );
}

pub async fn the_names_asked_for_are_those_kept(catalog: &Committing<'_, impl AccountNameCatalog>) {
    // given: three names kept
    for (account, name) in [
        ("account_hedy", "Hedy"),
        ("account_ada", "Ada"),
        ("account_otto", "Otto"),
    ] {
        named(catalog, account, name).await;
    }

    // expect: the names of those asked for that are kept, and none for an account never heard of
    assert_eq!(
        names_of(catalog, &["account_hedy", "account_ada", "account_unheard"]).await,
        names(&[("account_hedy", "Hedy"), ("account_ada", "Ada")])
    );
    // and: none for nobody asked for
    assert_eq!(names_of(catalog, &[]).await, names(&[]));
}

pub async fn a_deleted_account_s_name_is_forgotten_and_a_late_sign_up_does_not_bring_it_back(
    catalog: &Committing<'_, impl AccountNameCatalog>,
) {
    // given: two names kept
    named(catalog, "account_hedy", "Hedy").await;
    named(catalog, "account_ada", "Ada").await;

    // when: one account is deleted, and its sign-up arrives once more
    gone(catalog, "account_hedy", at(10)).await;
    named(catalog, "account_hedy", "Hedy").await;

    // then: its name is gone, and the other stays
    assert_eq!(
        names_of(catalog, &["account_hedy", "account_ada"]).await,
        names(&[("account_ada", "Ada")])
    );
}

pub async fn a_deletion_heard_before_the_sign_up_keeps_the_name_away(
    catalog: &Committing<'_, impl AccountNameCatalog>,
) {
    // when: an account's deletion arrives before its sign-up
    gone(catalog, "account_hedy", at(10)).await;
    named(catalog, "account_hedy", "Hedy").await;

    // then: its name is never kept
    assert_eq!(names_of(catalog, &["account_hedy"]).await, names(&[]));
}

pub async fn old_tombstones_are_forgotten_oldest_first_and_no_more_than_asked(
    catalog: &Committing<'_, impl AccountNameCatalog>,
) {
    // given: three accounts deleted one after another, and one more lately
    for (account, laid) in [
        ("account_first", 10),
        ("account_second", 20),
        ("account_third", 30),
        ("account_late", 1_000),
    ] {
        gone(catalog, account, at(laid)).await;
    }

    // when: tombstones laid before the late one are forgotten, two at a time
    let first = catalog
        .forget_name_tombstones_laid_before(at(500), 2)
        .await
        .expect("forgetting should succeed");

    // then: the two oldest are forgotten, so those accounts may be named again, and the third not
    assert_eq!(first, 2);
    for account in ["account_first", "account_second", "account_third"] {
        named(catalog, account, "Again").await;
    }
    assert_eq!(
        names_of(
            catalog,
            &["account_first", "account_second", "account_third"]
        )
        .await,
        names(&[("account_first", "Again"), ("account_second", "Again")])
    );

    // when: they are forgotten once more
    let then = catalog
        .forget_name_tombstones_laid_before(at(500), 2)
        .await
        .expect("forgetting should succeed");

    // then: only the one left before the late one is
    assert_eq!(then, 1);
    // and: the late tombstone still keeps its account's name away
    named(catalog, "account_late", "Late").await;
    assert_eq!(names_of(catalog, &["account_late"]).await, names(&[]));
}

macro_rules! names_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::names_suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! names_conformance_tests {
    ($workbench:ty) => {
        $crate::names_suite::names_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::names_suite::names_conformance_case!(
            $workbench,
            the_names_asked_for_are_those_kept
        );
        $crate::names_suite::names_conformance_case!(
            $workbench,
            a_deleted_account_s_name_is_forgotten_and_a_late_sign_up_does_not_bring_it_back
        );
        $crate::names_suite::names_conformance_case!(
            $workbench,
            a_deletion_heard_before_the_sign_up_keeps_the_name_away
        );
        $crate::names_suite::names_conformance_case!(
            $workbench,
            old_tombstones_are_forgotten_oldest_first_and_no_more_than_asked
        );
    };
}

pub(crate) use {names_conformance_case, names_conformance_tests};
