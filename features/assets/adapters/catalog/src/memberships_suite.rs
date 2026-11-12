use assets_core::{Membership, MembershipCatalog, ProjectLink};
use eventsourcing::AccountId;
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

async fn begun<W: Workbench>(bench: &W) -> Transaction {
    bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed")
}

async fn remembered<W: Workbench>(bench: &W, memberships: &[Membership]) {
    let mut transaction = begun(bench).await;
    for membership in memberships {
        bench
            .store()
            .remember(&mut transaction, membership)
            .await
            .expect("remembering should succeed");
    }
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn forgotten<W: Workbench>(bench: &W, membership: &Membership) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .forget(&mut transaction, membership)
        .await
        .expect("forgetting should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn project_forgotten<W: Workbench>(bench: &W, project: &ProjectLink) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .forget_project(&mut transaction, project)
        .await
        .expect("forgetting should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn share<W: Workbench>(bench: &W, one: &AccountId, other: &AccountId) -> bool {
    let mut transaction = begun(bench).await;
    let shared = bench
        .store()
        .share_a_project(&mut transaction, one, other)
        .await
        .expect("asking should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

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
            of(the_loom(), ada()),
            of(the_loom(), bo()),
            of(the_weft(), grace()),
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
            of(the_loom(), ada()),
            of(the_loom(), bo()),
            of(the_weft(), ada()),
            of(the_weft(), grace()),
        ],
    )
    .await;

    // when: Bo leaves the loom
    forgotten(bench, &of(the_loom(), bo())).await;

    // then: Bo shares nothing with Ada any more, and Grace still does
    assert!(!share(bench, &ada(), &bo()).await);
    assert!(share(bench, &ada(), &grace()).await);
}

pub async fn a_project_forgotten_is_shared_by_nobody_and_others_stay<W: Workbench>(bench: &W) {
    // given: Ada and Bo in the loom, Ada and Grace in the weft
    remembered(
        bench,
        &[
            of(the_loom(), ada()),
            of(the_loom(), bo()),
            of(the_weft(), ada()),
            of(the_weft(), grace()),
        ],
    )
    .await;

    // when: the loom is forgotten
    project_forgotten(bench, &the_loom()).await;

    // then: nobody shares the loom, and the weft is shared still
    assert!(!share(bench, &ada(), &bo()).await);
    assert!(share(bench, &ada(), &grace()).await);
}

pub async fn remembering_twice_or_forgetting_what_was_never_remembered_is_no_failure<
    W: Workbench,
>(
    bench: &W,
) {
    // given: Ada and Bo in the loom, Ada's membership remembered twice
    remembered(
        bench,
        &[
            of(the_loom(), ada()),
            of(the_loom(), ada()),
            of(the_loom(), bo()),
        ],
    )
    .await;

    // when: Grace's membership, never remembered, is forgotten, and a project never remembered
    forgotten(bench, &of(the_loom(), grace())).await;
    project_forgotten(bench, &the_weft()).await;

    // then: Ada and Bo share the loom still
    assert!(share(bench, &ada(), &bo()).await);
}

pub async fn what_is_remembered_in_a_transaction_that_is_dropped_is_not_kept<W: Workbench>(
    bench: &W,
) {
    // given: Ada and Bo remembered in the loom in a transaction never finished
    let mut transaction = begun(bench).await;
    for membership in [of(the_loom(), ada()), of(the_loom(), bo())] {
        bench
            .store()
            .remember(&mut transaction, &membership)
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
            a_project_forgotten_is_shared_by_nobody_and_others_stay
        );
        $crate::membership_conformance_case!(
            $workbench,
            remembering_twice_or_forgetting_what_was_never_remembered_is_no_failure
        );
        $crate::membership_conformance_case!(
            $workbench,
            what_is_remembered_in_a_transaction_that_is_dropped_is_not_kept
        );
    };
}
