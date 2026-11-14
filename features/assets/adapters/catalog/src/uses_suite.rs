use std::collections::HashSet;

use assets_core::{AssetId, AssetUse, AssetUses, Holder, Purpose};
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: AssetUses;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    async fn cleanup(self);
}

fn an_asset(seconds: i64) -> AssetId {
    AssetId::generate(
        ids::OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment"),
    )
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

fn account(id: &str) -> Holder {
    Holder::Account(AccountId::from(id))
}

fn picture_of(id: &str) -> AssetUse {
    AssetUse {
        purpose: Purpose::AccountPicture,
        by: account(id),
    }
}

fn ada_s_picture() -> AssetUse {
    picture_of("account_ada")
}

async fn begun<W: Workbench>(bench: &W) -> Transaction {
    bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed")
}

async fn finished(transaction: Transaction) {
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn used<W: Workbench>(bench: &W, asset: &AssetId, holder: &str, version: u64) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .use_as(
            &mut transaction,
            asset,
            Purpose::AccountPicture,
            &account(holder),
            Version::of(version),
        )
        .await
        .expect("storing a use should succeed");
    finished(transaction).await;
}

async fn stopped<W: Workbench>(bench: &W, holder: &str, version: u64, seconds: i64) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .stop_using(
            &mut transaction,
            Purpose::AccountPicture,
            &account(holder),
            Version::of(version),
            at(seconds),
        )
        .await
        .expect("stopping a use should succeed");
    finished(transaction).await;
}

async fn uses<W: Workbench>(bench: &W, assets: &[AssetId]) -> HashSet<(AssetId, AssetUse)> {
    let mut transaction = begun(bench).await;
    let uses = bench
        .store()
        .uses_of(&mut transaction, assets)
        .await
        .expect("asking should succeed");
    finished(transaction).await;

    uses.into_iter().collect()
}

async fn forgotten<W: Workbench>(bench: &W, before: i64, at_most: u64) -> u64 {
    let mut transaction = begun(bench).await;
    let forgotten = bench
        .store()
        .forget_tombstones_laid_before(&mut transaction, at(before), at_most)
        .await
        .expect("forgetting should succeed");
    finished(transaction).await;

    forgotten
}

pub async fn nothing_is_used_where_nothing_was_told<W: Workbench>(bench: &W) {
    // expect: no asset is used, and asking about none tells none
    assert!(uses(bench, &[an_asset(1)]).await.is_empty());
    assert!(uses(bench, &[]).await.is_empty());
}

pub async fn a_use_is_found_by_its_asset_and_another_s_by_its_own<W: Workbench>(bench: &W) {
    // given: Ada's picture is one asset, Bo's another, and a third is used by nobody
    let (hers, his, nobody_s) = (an_asset(1), an_asset(2), an_asset(3));
    used(bench, &hers, "account_ada", 2).await;
    used(bench, &his, "account_bo", 2).await;

    // when: the three are asked about
    let found = uses(bench, &[hers, his, nobody_s]).await;

    // then: each used one is found with its use, and the third not at all
    assert_eq!(
        found,
        HashSet::from([(hers, ada_s_picture()), (his, picture_of("account_bo")),])
    );
}

pub async fn a_newer_use_replaces_the_older_and_an_older_heard_late_changes_nothing<
    W: Workbench,
>(
    bench: &W,
) {
    // given: Ada's first picture, then a second at a later version
    let (first, second) = (an_asset(1), an_asset(2));
    used(bench, &first, "account_ada", 2).await;
    used(bench, &second, "account_ada", 3).await;

    // when: the first is told again, late, at its older version
    used(bench, &first, "account_ada", 2).await;

    // then: only the second is used
    assert_eq!(
        uses(bench, &[first, second]).await,
        HashSet::from([(second, ada_s_picture())])
    );
}

pub async fn a_use_stopped_is_found_no_more_and_not_brought_back_late<W: Workbench>(bench: &W) {
    // given: Ada's picture, then no picture at a later version
    let picture = an_asset(1);
    used(bench, &picture, "account_ada", 2).await;
    stopped(bench, "account_ada", 3, 1_000).await;

    // then: the picture is used no more
    assert!(uses(bench, &[picture]).await.is_empty());
    // when: the use is told again, late, at its older version
    used(bench, &picture, "account_ada", 2).await;
    // then: it stays unused
    assert!(uses(bench, &[picture]).await.is_empty());
    // when: a newer picture comes
    used(bench, &picture, "account_ada", 4).await;
    // then: it is used again
    assert_eq!(
        uses(bench, &[picture]).await,
        HashSet::from([(picture, ada_s_picture())])
    );
}

pub async fn tombstones_laid_long_ago_are_let_go_and_recent_ones_still_guard<W: Workbench>(
    bench: &W,
) {
    // given: Ada stopped using a picture long ago, Bo lately, and Grace still uses one
    let (hers, his, grace_s) = (an_asset(1), an_asset(2), an_asset(3));
    used(bench, &hers, "account_ada", 2).await;
    stopped(bench, "account_ada", 3, 1_000).await;
    used(bench, &his, "account_bo", 2).await;
    stopped(bench, "account_bo", 3, 5_000).await;
    used(bench, &grace_s, "account_grace", 2).await;

    // when: the tombstones laid before a time between the two are let go
    let let_go = forgotten(bench, 3_000, 10).await;

    // then: Ada's alone is let go
    assert_eq!(let_go, 1);
    // and: a late word on Ada's old picture brings it back, while Bo's is still guarded
    used(bench, &hers, "account_ada", 2).await;
    used(bench, &his, "account_bo", 2).await;
    assert_eq!(
        uses(bench, &[hers, his, grace_s]).await,
        HashSet::from([
            (hers, ada_s_picture()),
            (grace_s, picture_of("account_grace")),
        ])
    );
    // and: a use never stopped is no tombstone
    assert_eq!(forgotten(bench, 10_000, 10).await, 1);
}

pub async fn a_use_told_in_a_transaction_that_is_dropped_is_not_kept<W: Workbench>(bench: &W) {
    // given: a use told in a transaction that is dropped
    let picture = an_asset(1);
    {
        let mut transaction = begun(bench).await;
        bench
            .store()
            .use_as(
                &mut transaction,
                &picture,
                Purpose::AccountPicture,
                &account("account_ada"),
                Version::of(2),
            )
            .await
            .expect("storing a use should succeed");
    }

    // expect: nothing is used
    assert!(uses(bench, &[picture]).await.is_empty());
}

#[macro_export]
macro_rules! uses_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::uses_suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::uses_suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! uses_conformance_tests {
    ($workbench:ty) => {
        $crate::uses_conformance_case!($workbench, nothing_is_used_where_nothing_was_told);
        $crate::uses_conformance_case!(
            $workbench,
            a_use_is_found_by_its_asset_and_another_s_by_its_own
        );
        $crate::uses_conformance_case!(
            $workbench,
            a_newer_use_replaces_the_older_and_an_older_heard_late_changes_nothing
        );
        $crate::uses_conformance_case!(
            $workbench,
            a_use_stopped_is_found_no_more_and_not_brought_back_late
        );
        $crate::uses_conformance_case!(
            $workbench,
            tombstones_laid_long_ago_are_let_go_and_recent_ones_still_guard
        );
        $crate::uses_conformance_case!(
            $workbench,
            a_use_told_in_a_transaction_that_is_dropped_is_not_kept
        );
    };
}
