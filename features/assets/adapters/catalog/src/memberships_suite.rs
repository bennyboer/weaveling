use assets_core::{Membership, MembershipCatalog, ProjectLink};
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: MembershipCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    async fn cleanup(self);
}

fn ada() -> AccountId {
    AccountId::from("account_ada")
}

fn bo() -> AccountId {
    AccountId::from("account_bo")
}

fn grace() -> AccountId {
    AccountId::from("account_grace")
}

fn the_loom() -> ProjectLink {
    ProjectLink::from("project_loom")
}

fn the_weft() -> ProjectLink {
    ProjectLink::from("project_weft")
}

fn of(project: ProjectLink, account: AccountId) -> Membership {
    Membership { project, account }
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
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

async fn remembered<W: Workbench>(bench: &W, memberships: &[(Membership, u64)]) {
    let mut transaction = begun(bench).await;
    for (membership, version) in memberships {
        bench
            .store()
            .remember(&mut transaction, membership, Version::of(*version))
            .await
            .expect("remembering should succeed");
    }
    finished(transaction).await;
}

async fn forgotten<W: Workbench>(bench: &W, membership: &Membership, version: u64, when: i64) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .forget(&mut transaction, membership, Version::of(version), at(when))
        .await
        .expect("forgetting should succeed");
    finished(transaction).await;
}

async fn disbanded<W: Workbench>(bench: &W, project: &ProjectLink, version: u64, when: i64) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .forget_project(&mut transaction, project, Version::of(version), at(when))
        .await
        .expect("forgetting should succeed");
    finished(transaction).await;
}

async fn tombstones_forgotten<W: Workbench>(bench: &W, before: i64, at_most: u64) -> u64 {
    let mut transaction = begun(bench).await;
    let forgotten = bench
        .store()
        .forget_tombstones_laid_before(&mut transaction, at(before), at_most)
        .await
        .expect("forgetting should succeed");
    finished(transaction).await;

    forgotten
}

async fn share<W: Workbench>(bench: &W, one: &AccountId, other: &AccountId) -> bool {
    let mut transaction = begun(bench).await;
    let shared = bench
        .store()
        .share_a_project(&mut transaction, one, other)
        .await
        .expect("asking should succeed");
    finished(transaction).await;

    shared
}

pub async fn accounts_in_no_project_share_none<W: Workbench>(bench: &W) {
    // expect: nobody shares a project where nobody is a member
    assert!(!share(bench, &ada(), &bo()).await);
}

pub async fn members_of_one_project_share_it_and_others_do_not<W: Workbench>(bench: &W) {
    // given: Ada and Bo in the loom, Grace in the weft alone
    remembered(
        bench,
        &[
            (of(the_loom(), ada()), 1),
            (of(the_loom(), bo()), 2),
            (of(the_weft(), grace()), 1),
        ],
    )
    .await;

    // expect: Ada and Bo share a project, either way round
    assert!(share(bench, &ada(), &bo()).await);
    assert!(share(bench, &bo(), &ada()).await);
    // and: Grace shares none with either
    assert!(!share(bench, &ada(), &grace()).await);
    assert!(!share(bench, &grace(), &bo()).await);
}

pub async fn one_who_left_shares_no_more_and_the_rest_stay<W: Workbench>(bench: &W) {
    // given: Ada and Bo in the loom, Ada and Grace in the weft
    remembered(
        bench,
        &[
            (of(the_loom(), ada()), 1),
            (of(the_loom(), bo()), 2),
            (of(the_weft(), ada()), 1),
            (of(the_weft(), grace()), 2),
        ],
    )
    .await;

    // when: Bo leaves the loom
    forgotten(bench, &of(the_loom(), bo()), 3, 1_000).await;

    // then: Bo shares nothing with Ada any more, and Grace still does
    assert!(!share(bench, &ada(), &bo()).await);
    assert!(share(bench, &ada(), &grace()).await);
}

pub async fn a_project_disbanded_is_shared_by_nobody_and_others_stay<W: Workbench>(bench: &W) {
    // given: Ada and Bo in the loom, Ada and Grace in the weft
    remembered(
        bench,
        &[
            (of(the_loom(), ada()), 1),
            (of(the_loom(), bo()), 2),
            (of(the_weft(), ada()), 1),
            (of(the_weft(), grace()), 2),
        ],
    )
    .await;

    // when: the loom is disbanded
    disbanded(bench, &the_loom(), 3, 1_000).await;

    // then: nobody shares the loom, and the weft is shared still
    assert!(!share(bench, &ada(), &bo()).await);
    assert!(share(bench, &ada(), &grace()).await);
}

pub async fn a_word_heard_twice_or_of_what_was_never_remembered_is_no_failure<W: Workbench>(
    bench: &W,
) {
    // given: Ada and Bo in the loom, Ada's joining heard twice
    remembered(
        bench,
        &[
            (of(the_loom(), ada()), 1),
            (of(the_loom(), ada()), 1),
            (of(the_loom(), bo()), 2),
        ],
    )
    .await;

    // when: Grace's leaving, never remembered joined, is heard, and the weft disbanded twice
    forgotten(bench, &of(the_loom(), grace()), 3, 1_000).await;
    disbanded(bench, &the_weft(), 1, 1_000).await;
    disbanded(bench, &the_weft(), 1, 1_000).await;

    // then: Ada and Bo share the loom still
    assert!(share(bench, &ada(), &bo()).await);
}

pub async fn a_joining_heard_after_the_leaving_that_followed_it_changes_nothing<W: Workbench>(
    bench: &W,
) {
    // given: Ada in the loom, and Bo's leaving heard before his joining
    remembered(bench, &[(of(the_loom(), ada()), 1)]).await;
    forgotten(bench, &of(the_loom(), bo()), 3, 1_000).await;

    // when: his joining is heard late
    remembered(bench, &[(of(the_loom(), bo()), 2)]).await;

    // then: he shares nothing with Ada
    assert!(
        !share(bench, &ada(), &bo()).await,
        "a word heard late must not undo a newer one"
    );
}

pub async fn one_who_joins_again_shares_again<W: Workbench>(bench: &W) {
    // given: Ada in the loom, and Bo who joined and left
    remembered(
        bench,
        &[(of(the_loom(), ada()), 1), (of(the_loom(), bo()), 2)],
    )
    .await;
    forgotten(bench, &of(the_loom(), bo()), 3, 1_000).await;

    // when: he joins again
    remembered(bench, &[(of(the_loom(), bo()), 4)]).await;

    // then: he shares the loom with Ada again
    assert!(share(bench, &ada(), &bo()).await);
}

pub async fn a_joining_heard_after_the_project_was_disbanded_changes_nothing<W: Workbench>(
    bench: &W,
) {
    // given: Ada in the loom, which was disbanded
    remembered(bench, &[(of(the_loom(), ada()), 1)]).await;
    disbanded(bench, &the_loom(), 3, 1_000).await;

    // when: Bo's joining is heard late
    remembered(bench, &[(of(the_loom(), bo()), 2)]).await;

    // then: nobody shares the loom
    assert!(!share(bench, &ada(), &bo()).await);
}

pub async fn tombstones_laid_long_ago_are_let_go_and_those_laid_lately_still_guard<W: Workbench>(
    bench: &W,
) {
    // given: Ada in the loom and the weft, Bo gone from the loom long ago, Grace gone from it lately
    remembered(
        bench,
        &[
            (of(the_loom(), ada()), 1),
            (of(the_weft(), ada()), 1),
            (of(the_weft(), bo()), 2),
        ],
    )
    .await;
    forgotten(bench, &of(the_loom(), bo()), 3, 1_000).await;
    forgotten(bench, &of(the_loom(), grace()), 4, 5_000).await;
    // and: the weft disbanded long ago
    disbanded(bench, &the_weft(), 3, 1_000).await;

    // when: what was laid before a moment between is let go
    let forgotten = tombstones_forgotten(bench, 3_000, 10).await;

    // then: Bo's leaving and the weft's disbanding are let go, Grace's leaving is not
    assert_eq!(forgotten, 2);
    assert_eq!(tombstones_forgotten(bench, 3_000, 10).await, 0);
    // and: an older word about Bo is taken in once nothing guards against it
    remembered(bench, &[(of(the_loom(), bo()), 2)]).await;
    assert!(share(bench, &ada(), &bo()).await);
    // and: Grace's leaving still guards against one about her
    remembered(bench, &[(of(the_loom(), grace()), 2)]).await;
    assert!(!share(bench, &ada(), &grace()).await);
}

pub async fn no_more_tombstones_than_asked_are_let_go_at_once<W: Workbench>(bench: &W) {
    // given: three memberships gone long ago
    for (account, version) in [(ada(), 1), (bo(), 2), (grace(), 3)] {
        forgotten(bench, &of(the_loom(), account), version, 1_000).await;
    }

    // expect: two are let go, then the third
    assert_eq!(tombstones_forgotten(bench, 3_000, 2).await, 2);
    assert_eq!(tombstones_forgotten(bench, 3_000, 2).await, 1);
}

pub async fn what_is_remembered_in_a_transaction_that_is_dropped_is_not_kept<W: Workbench>(
    bench: &W,
) {
    // given: Ada and Bo remembered in the loom in a transaction never finished
    let mut transaction = begun(bench).await;
    for (membership, version) in [(of(the_loom(), ada()), 1), (of(the_loom(), bo()), 2)] {
        bench
            .store()
            .remember(&mut transaction, &membership, Version::of(version))
            .await
            .expect("remembering should succeed");
    }

    // when: the transaction is dropped
    drop(transaction);

    // then: they share nothing
    assert!(!share(bench, &ada(), &bo()).await);
}

#[macro_export]
macro_rules! membership_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::memberships_suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::memberships_suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! membership_conformance_tests {
    ($workbench:ty) => {
        $crate::membership_conformance_case!($workbench, accounts_in_no_project_share_none);
        $crate::membership_conformance_case!(
            $workbench,
            members_of_one_project_share_it_and_others_do_not
        );
        $crate::membership_conformance_case!(
            $workbench,
            one_who_left_shares_no_more_and_the_rest_stay
        );
        $crate::membership_conformance_case!(
            $workbench,
            a_project_disbanded_is_shared_by_nobody_and_others_stay
        );
        $crate::membership_conformance_case!(
            $workbench,
            a_word_heard_twice_or_of_what_was_never_remembered_is_no_failure
        );
        $crate::membership_conformance_case!(
            $workbench,
            a_joining_heard_after_the_leaving_that_followed_it_changes_nothing
        );
        $crate::membership_conformance_case!($workbench, one_who_joins_again_shares_again);
        $crate::membership_conformance_case!(
            $workbench,
            a_joining_heard_after_the_project_was_disbanded_changes_nothing
        );
        $crate::membership_conformance_case!(
            $workbench,
            tombstones_laid_long_ago_are_let_go_and_those_laid_lately_still_guard
        );
        $crate::membership_conformance_case!(
            $workbench,
            no_more_tombstones_than_asked_are_let_go_at_once
        );
        $crate::membership_conformance_case!(
            $workbench,
            what_is_remembered_in_a_transaction_that_is_dropped_is_not_kept
        );
    };
}
