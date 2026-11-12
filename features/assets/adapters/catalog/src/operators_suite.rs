use assets_core::OperatorCatalog;
use eventsourcing::{AccountId, Version};
use transacting::{Transaction, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: OperatorCatalog;

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

async fn begun<W: Workbench>(bench: &W) -> Transaction {
    bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed")
}

async fn remembered<W: Workbench>(bench: &W, account: &AccountId, version: u64, operator: bool) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .remember(&mut transaction, account, Version::of(version), operator)
        .await
        .expect("remembering should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn operates<W: Workbench>(bench: &W, account: &AccountId) -> bool {
    let mut transaction = begun(bench).await;
    let operates = bench
        .store()
        .operates(&mut transaction, account)
        .await
        .expect("asking should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    operates
}

pub async fn an_account_never_told_of_runs_nothing<W: Workbench>(bench: &W) {
    // expect: Ada does not run the instance
    assert!(!operates(bench, &ada()).await);
}

pub async fn an_account_told_an_operator_runs_the_instance_and_no_other<W: Workbench>(bench: &W) {
    // given: Ada became an operator
    remembered(bench, &ada(), 3, true).await;

    // expect: she runs the instance, and Bo does not
    assert!(operates(bench, &ada()).await);
    assert!(!operates(bench, &bo()).await);
}

pub async fn an_operator_who_stopped_runs_it_no_more_and_may_again<W: Workbench>(bench: &W) {
    // given: Ada became an operator, and stopped
    remembered(bench, &ada(), 3, true).await;
    remembered(bench, &ada(), 5, false).await;

    // expect: she runs it no more
    assert!(!operates(bench, &ada()).await);
    // when: she becomes one again
    remembered(bench, &ada(), 7, true).await;
    // then: she runs it again
    assert!(operates(bench, &ada()).await);
}

pub async fn a_word_heard_after_a_newer_one_changes_nothing<W: Workbench>(bench: &W) {
    // given: Ada's stopping heard before her becoming an operator
    remembered(bench, &ada(), 5, false).await;
    remembered(bench, &ada(), 3, true).await;
    // and: Bo's becoming one heard twice
    remembered(bench, &bo(), 2, true).await;
    remembered(bench, &bo(), 2, true).await;

    // expect: Ada does not run the instance, and Bo does
    assert!(
        !operates(bench, &ada()).await,
        "a word heard late must not undo a newer one"
    );
    assert!(operates(bench, &bo()).await);
}

pub async fn what_is_remembered_in_a_transaction_that_is_dropped_is_not_kept<W: Workbench>(
    bench: &W,
) {
    // given: Ada remembered an operator in a transaction never finished
    let mut transaction = begun(bench).await;
    bench
        .store()
        .remember(&mut transaction, &ada(), Version::of(3), true)
        .await
        .expect("remembering should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: she runs nothing
    assert!(!operates(bench, &ada()).await);
}

#[macro_export]
macro_rules! operators_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::operators_suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::operators_suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! operators_conformance_tests {
    ($workbench:ty) => {
        $crate::operators_conformance_case!($workbench, an_account_never_told_of_runs_nothing);
        $crate::operators_conformance_case!(
            $workbench,
            an_account_told_an_operator_runs_the_instance_and_no_other
        );
        $crate::operators_conformance_case!(
            $workbench,
            an_operator_who_stopped_runs_it_no_more_and_may_again
        );
        $crate::operators_conformance_case!(
            $workbench,
            a_word_heard_after_a_newer_one_changes_nothing
        );
        $crate::operators_conformance_case!(
            $workbench,
            what_is_remembered_in_a_transaction_that_is_dropped_is_not_kept
        );
    };
}
