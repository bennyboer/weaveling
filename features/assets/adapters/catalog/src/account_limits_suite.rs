use assets_core::AccountLimits;
use eventsourcing::AccountId;
use transacting::{Transaction, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: AccountLimits;

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

async fn finished(transaction: Transaction) {
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn given<W: Workbench>(bench: &W, account: &AccountId, most: u64) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .give(&mut transaction, account, most)
        .await
        .expect("giving should succeed");
    finished(transaction).await;
}

async fn taken_back<W: Workbench>(bench: &W, account: &AccountId) {
    let mut transaction = begun(bench).await;
    bench
        .store()
        .take_back(&mut transaction, account)
        .await
        .expect("taking back should succeed");
    finished(transaction).await;
}

async fn limit_of<W: Workbench>(bench: &W, account: &AccountId) -> Option<u64> {
    let mut transaction = begun(bench).await;
    let limit = bench
        .store()
        .limit_of(&mut transaction, account)
        .await
        .expect("asking should succeed");
    finished(transaction).await;

    limit
}

pub async fn an_account_given_nothing_has_no_limit_of_its_own<W: Workbench>(bench: &W) {
    // expect: no limit of Ada's own
    assert_eq!(limit_of(bench, &ada()).await, None);
}

pub async fn a_limit_given_is_the_account_s_alone_and_the_last_given_holds<W: Workbench>(
    bench: &W,
) {
    // given: Ada given five gigabytes, and then two
    given(bench, &ada(), 5 << 30).await;
    given(bench, &ada(), 2 << 30).await;

    // expect: Ada's limit is the last given, and Bo has none
    assert_eq!(limit_of(bench, &ada()).await, Some(2 << 30));
    assert_eq!(limit_of(bench, &bo()).await, None);
}

pub async fn a_limit_taken_back_is_no_more_and_one_never_given_is_no_failure<W: Workbench>(
    bench: &W,
) {
    // given: Ada and Bo given limits of their own
    given(bench, &ada(), 100).await;
    given(bench, &bo(), 200).await;

    // when: Ada's is taken back, twice
    taken_back(bench, &ada()).await;
    taken_back(bench, &ada()).await;

    // then: Ada has none of her own, and Bo still his
    assert_eq!(limit_of(bench, &ada()).await, None);
    assert_eq!(limit_of(bench, &bo()).await, Some(200));
}

pub async fn a_limit_as_large_as_can_be_counted_is_kept_whole<W: Workbench>(bench: &W) {
    // given: Ada given the most a signed count of bytes holds
    given(bench, &ada(), i64::MAX as u64).await;

    // expect: it is kept whole
    assert_eq!(limit_of(bench, &ada()).await, Some(i64::MAX as u64));
}

pub async fn a_limit_given_in_a_transaction_that_is_dropped_is_not_kept<W: Workbench>(bench: &W) {
    // given: Ada given a limit in a transaction never finished
    let mut transaction = begun(bench).await;
    bench
        .store()
        .give(&mut transaction, &ada(), 100)
        .await
        .expect("giving should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: she has none of her own
    assert_eq!(limit_of(bench, &ada()).await, None);
}

#[macro_export]
macro_rules! account_limits_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::account_limits_suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::account_limits_suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! account_limits_conformance_tests {
    ($workbench:ty) => {
        $crate::account_limits_conformance_case!(
            $workbench,
            an_account_given_nothing_has_no_limit_of_its_own
        );
        $crate::account_limits_conformance_case!(
            $workbench,
            a_limit_given_is_the_account_s_alone_and_the_last_given_holds
        );
        $crate::account_limits_conformance_case!(
            $workbench,
            a_limit_taken_back_is_no_more_and_one_never_given_is_no_failure
        );
        $crate::account_limits_conformance_case!(
            $workbench,
            a_limit_as_large_as_can_be_counted_is_kept_whole
        );
        $crate::account_limits_conformance_case!(
            $workbench,
            a_limit_given_in_a_transaction_that_is_dropped_is_not_kept
        );
    };
}
