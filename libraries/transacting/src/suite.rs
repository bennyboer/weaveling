use std::sync::{Arc, Mutex};

use crate::{Joined, Transaction, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Transactions: Transactions;

    async fn setup() -> Self;

    fn transactions(&self) -> &Self::Transactions;

    async fn write(&self, transaction: &mut Transaction, word: &str);

    async fn written(&self) -> Vec<String>;

    async fn cleanup(self);
}

pub async fn what_a_finished_transaction_wrote_is_kept(bench: &impl Workbench) {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench.write(&mut transaction, "warp").await;
    bench.write(&mut transaction, "weft").await;

    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    assert_eq!(bench.written().await, vec!["warp", "weft"]);
}

pub async fn what_a_dropped_transaction_wrote_is_gone(bench: &impl Workbench) {
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench.write(&mut transaction, "warp").await;
    bench.write(&mut transaction, "weft").await;

    drop(transaction);

    assert!(
        bench.written().await.is_empty(),
        "a transaction not finished leaves nothing behind"
    );
}

pub async fn what_was_finished_before_stays_when_a_later_transaction_is_dropped(
    bench: &impl Workbench,
) {
    let mut kept = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench.write(&mut kept, "warp").await;
    kept.finish().await.expect("finishing should succeed");
    let mut dropped = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench.write(&mut dropped, "weft").await;

    drop(dropped);

    assert_eq!(bench.written().await, vec!["warp"]);
}

pub async fn joining_none_begins_a_transaction_its_joiner_finishes(bench: &impl Workbench) {
    let mut joined = bench
        .transactions()
        .join(None)
        .await
        .expect("joining should succeed");
    assert!(matches!(joined, Joined::Own(_)));
    bench.write(&mut joined, "warp").await;

    joined.finish().await.expect("finishing should succeed");

    assert_eq!(bench.written().await, vec!["warp"]);
}

pub async fn a_borrowed_transaction_is_not_finished_by_whoever_joined_it(bench: &impl Workbench) {
    let mut owned = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    let mut joined = bench
        .transactions()
        .join(Some(&mut owned))
        .await
        .expect("joining should succeed");
    bench.write(&mut joined, "warp").await;
    joined.finish().await.expect("finishing should succeed");

    drop(owned);

    assert!(
        bench.written().await.is_empty(),
        "only the owner of a transaction finishes it"
    );
}

pub async fn a_borrowed_transaction_keeps_what_was_written_in_it_once_its_owner_finishes(
    bench: &impl Workbench,
) {
    let mut owned = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench.write(&mut owned, "warp").await;
    let mut joined = bench
        .transactions()
        .join(Some(&mut owned))
        .await
        .expect("joining should succeed");
    bench.write(&mut joined, "weft").await;
    joined.finish().await.expect("finishing should succeed");

    owned.finish().await.expect("finishing should succeed");

    assert_eq!(bench.written().await, vec!["warp", "weft"]);
}

struct Followups(Arc<Mutex<Vec<&'static str>>>);

impl Followups {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn noting(&self, what: &'static str) -> impl FnOnce() + Send + 'static {
        let ran = self.0.clone();

        move || ran.lock().expect("nothing panics holding it").push(what)
    }

    fn ran(&self) -> Vec<&'static str> {
        self.0.lock().expect("nothing panics holding it").clone()
    }
}

pub async fn what_follows_a_commit_runs_once_it_is_finished_in_the_order_it_was_asked(
    bench: &impl Workbench,
) {
    let followups = Followups::new();
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench.write(&mut transaction, "warp").await;
    transaction.after_commit(followups.noting("woken"));
    transaction.after_commit(followups.noting("told"));
    let before = followups.ran();

    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    assert_eq!(
        (before, followups.ran()),
        (Vec::new(), vec!["woken", "told"]),
        "nothing follows a commit before it happened"
    );
}

pub async fn what_follows_a_commit_never_runs_when_the_transaction_is_dropped(
    bench: &impl Workbench,
) {
    let followups = Followups::new();
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench.write(&mut transaction, "warp").await;
    transaction.after_commit(followups.noting("woken"));

    drop(transaction);

    assert!(followups.ran().is_empty());
}

pub async fn what_follows_a_commit_asked_through_a_borrowed_transaction_waits_for_its_owner(
    bench: &impl Workbench,
) {
    let followups = Followups::new();
    let mut owned = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    let mut joined = bench
        .transactions()
        .join(Some(&mut owned))
        .await
        .expect("joining should succeed");
    joined.after_commit(followups.noting("woken"));
    joined.finish().await.expect("finishing should succeed");
    let after_joined = followups.ran();

    owned.finish().await.expect("finishing should succeed");

    assert_eq!((after_joined, followups.ran()), (Vec::new(), vec!["woken"]));
}

#[doc(hidden)]
#[macro_export]
macro_rules! transacting_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::transacting_conformance_case!(
            $workbench,
            what_a_finished_transaction_wrote_is_kept
        );
        $crate::transacting_conformance_case!($workbench, what_a_dropped_transaction_wrote_is_gone);
        $crate::transacting_conformance_case!(
            $workbench,
            what_was_finished_before_stays_when_a_later_transaction_is_dropped
        );
        $crate::transacting_conformance_case!(
            $workbench,
            joining_none_begins_a_transaction_its_joiner_finishes
        );
        $crate::transacting_conformance_case!(
            $workbench,
            a_borrowed_transaction_is_not_finished_by_whoever_joined_it
        );
        $crate::transacting_conformance_case!(
            $workbench,
            a_borrowed_transaction_keeps_what_was_written_in_it_once_its_owner_finishes
        );
        $crate::transacting_conformance_case!(
            $workbench,
            what_follows_a_commit_runs_once_it_is_finished_in_the_order_it_was_asked
        );
        $crate::transacting_conformance_case!(
            $workbench,
            what_follows_a_commit_never_runs_when_the_transaction_is_dropped
        );
        $crate::transacting_conformance_case!(
            $workbench,
            what_follows_a_commit_asked_through_a_borrowed_transaction_waits_for_its_owner
        );
    };
}
