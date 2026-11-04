#![cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]

use time::{Duration, OffsetDateTime};
use transacting::{Transaction, Transactions};

use crate::{Trash, Trashed};

const PROJECT: &str = "project";
const ACCOUNT: &str = "account";

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Trash: Trash;

    async fn setup() -> Self;

    fn trash(&self) -> &Self::Trash;

    fn transactions(&self) -> &dyn Transactions;

    async fn cleanup(self);
}

pub fn at(days: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_700_000_000) + Duration::days(days)
}

async fn begun<W: Workbench>(bench: &W) -> Transaction {
    bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed")
}

async fn put_into<W: Workbench>(bench: &W, kind: &str, id: &str, until: OffsetDateTime) {
    let mut transaction = begun(bench).await;
    bench
        .trash()
        .put_into(&mut transaction, kind, id, until)
        .await
        .expect("putting into the trash should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

async fn taken_out<W: Workbench>(bench: &W, kind: &str, id: &str) -> bool {
    let mut transaction = begun(bench).await;
    let taken = bench
        .trash()
        .take_out(&mut transaction, kind, id)
        .await
        .expect("taking out should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    taken
}

async fn due<W: Workbench>(bench: &W, at: OffsetDateTime, at_most: u64) -> Vec<Trashed> {
    bench
        .trash()
        .due(at, at_most)
        .await
        .expect("asking what is due should succeed")
}

fn trashed(kind: &str, id: &str) -> Trashed {
    Trashed {
        kind: kind.to_owned(),
        id: id.to_owned(),
    }
}

pub async fn what_is_put_in_is_due_once_its_time_has_come<W: Workbench>() {
    // given: a trash
    let bench = W::setup().await;

    // when: something is put in until a moment
    put_into(&bench, PROJECT, "project_1", at(30)).await;

    // then: it is not due before that moment
    assert!(due(&bench, at(29), 10).await.is_empty());
    // and: due at it
    assert_eq!(
        due(&bench, at(30), 10).await,
        [trashed(PROJECT, "project_1")],
        "due at the very moment it is to be purged"
    );
    bench.cleanup().await;
}

pub async fn what_is_due_says_its_kind_and_one_id_of_two_kinds_is_two_things<W: Workbench>() {
    // given: a trash
    let bench = W::setup().await;

    // when: one id is put in as two kinds
    put_into(&bench, PROJECT, "thing_1", at(30)).await;
    put_into(&bench, ACCOUNT, "thing_1", at(30)).await;
    // and: taken out as one of them
    taken_out(&bench, ACCOUNT, "thing_1").await;
    // and: something else of that kind is put in
    put_into(&bench, ACCOUNT, "account_1", at(30)).await;

    // then: what is due is the other kind's and the something else, each with its kind
    assert_eq!(
        due(&bench, at(30), 10).await,
        [trashed(ACCOUNT, "account_1"), trashed(PROJECT, "thing_1")]
    );
    bench.cleanup().await;
}

pub async fn what_is_due_comes_oldest_first_and_no_more_than_asked_for<W: Workbench>() {
    // given: a trash
    let bench = W::setup().await;

    // when: four things are put in out of order
    for (kind, id, days) in [
        (PROJECT, "project_c", 3),
        (ACCOUNT, "account_a", 1),
        (PROJECT, "project_d", 4),
        (PROJECT, "project_b", 2),
    ] {
        put_into(&bench, kind, id, at(days)).await;
    }

    // then: the three oldest are due, oldest first
    assert_eq!(
        due(&bench, at(10), 3).await,
        [
            trashed(ACCOUNT, "account_a"),
            trashed(PROJECT, "project_b"),
            trashed(PROJECT, "project_c")
        ]
    );
    bench.cleanup().await;
}

pub async fn what_is_taken_out_is_due_no_more<W: Workbench>() {
    // given: something in the trash
    let bench = W::setup().await;
    put_into(&bench, PROJECT, "project_1", at(30)).await;

    // when: it is taken out twice
    let taken = (
        taken_out(&bench, PROJECT, "project_1").await,
        taken_out(&bench, PROJECT, "project_1").await,
    );

    // then: only the first takes it
    assert_eq!(taken, (true, false), "only what was kept can be taken out");
    // and: it is not due
    assert!(due(&bench, at(30), 10).await.is_empty());
    bench.cleanup().await;
}

pub async fn putting_in_again_keeps_it_until_the_later_moment<W: Workbench>() {
    // given: a trash
    let bench = W::setup().await;

    // when: something is put in and put in again until later
    put_into(&bench, PROJECT, "project_1", at(30)).await;
    put_into(&bench, PROJECT, "project_1", at(60)).await;

    // then: it is not due at the earlier moment
    assert!(due(&bench, at(30), 10).await.is_empty());
    // and: due at the later
    assert_eq!(
        due(&bench, at(60), 10).await,
        [trashed(PROJECT, "project_1")]
    );
    bench.cleanup().await;
}

pub async fn what_is_put_in_in_a_dropped_transaction_was_never_put_in<W: Workbench>() {
    // given: something put in within a transaction
    let bench = W::setup().await;
    let mut transaction = begun(&bench).await;
    bench
        .trash()
        .put_into(&mut transaction, PROJECT, "project_1", at(30))
        .await
        .expect("putting into the trash should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: nothing is due
    assert!(due(&bench, at(30), 10).await.is_empty());
    bench.cleanup().await;
}

pub async fn what_is_taken_out_in_a_dropped_transaction_is_still_kept<W: Workbench>() {
    // given: something in the trash
    let bench = W::setup().await;
    put_into(&bench, PROJECT, "project_1", at(30)).await;
    // and: taken out within a transaction
    let mut transaction = begun(&bench).await;
    bench
        .trash()
        .take_out(&mut transaction, PROJECT, "project_1")
        .await
        .expect("taking out should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: it is still due
    assert_eq!(
        due(&bench, at(30), 10).await,
        [trashed(PROJECT, "project_1")]
    );
    bench.cleanup().await;
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::conformance_tests!($workbench:
            what_is_put_in_is_due_once_its_time_has_come,
            what_is_due_says_its_kind_and_one_id_of_two_kinds_is_two_things,
            what_is_due_comes_oldest_first_and_no_more_than_asked_for,
            what_is_taken_out_is_due_no_more,
            putting_in_again_keeps_it_until_the_later_moment,
            what_is_put_in_in_a_dropped_transaction_was_never_put_in,
            what_is_taken_out_in_a_dropped_transaction_is_still_kept,
        );
    };
    ($workbench:ty: $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test(flavor = "multi_thread")]
            async fn $case() {
                $crate::suite::$case::<$workbench>().await;
            }
        )+
    };
}
