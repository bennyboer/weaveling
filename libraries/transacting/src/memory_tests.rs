use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use crate::memory;
use crate::suite::Workbench;
use crate::{InMemoryTransactions, Transaction, Transactions};

struct InMemory {
    transactions: InMemoryTransactions,
    words: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Workbench for InMemory {
    type Transactions = InMemoryTransactions;

    async fn setup() -> Self {
        Self {
            transactions: InMemoryTransactions::new(),
            words: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn transactions(&self) -> &Self::Transactions {
        &self.transactions
    }

    async fn write(&self, transaction: &mut Transaction, word: &str) {
        self.words
            .lock()
            .expect("nothing panics holding the words")
            .push(word.to_owned());
        let words = self.words.clone();
        memory::undo_on_rollback(transaction, move || {
            words
                .lock()
                .expect("nothing panics holding the words")
                .pop();
        })
        .expect("a transaction begun in memory takes an undo");
    }

    async fn written(&self) -> Vec<String> {
        self.words
            .lock()
            .expect("nothing panics holding the words")
            .clone()
    }

    async fn cleanup(self) {}
}

crate::conformance_tests!(InMemory);

const A_MOMENT: Duration = Duration::from_millis(50);

#[tokio::test]
async fn undos_run_last_first() {
    // given: a transaction that took three undos in turn
    let transactions = InMemoryTransactions::new();
    let undone = Arc::new(Mutex::new(Vec::new()));
    let mut transaction = transactions
        .begin()
        .await
        .expect("beginning should succeed");
    for nth in 1..=3 {
        let undone = undone.clone();
        memory::undo_on_rollback(&mut transaction, move || {
            undone.lock().expect("nothing panics holding it").push(nth);
        })
        .expect("a transaction begun in memory takes an undo");
    }

    // when: it is dropped
    drop(transaction);

    // then: they run last first
    assert_eq!(
        *undone.lock().expect("nothing panics holding it"),
        vec![3, 2, 1],
        "a later write may build on an earlier one, so it is undone first"
    );
}

#[tokio::test]
async fn a_second_transaction_begins_only_once_the_first_is_over() {
    // given: a transaction begun in memory
    let transactions = InMemoryTransactions::new();
    let first = transactions
        .begin()
        .await
        .expect("beginning should succeed");

    // when: a second is begun while it lasts
    let waiting = tokio::spawn({
        let transactions = transactions.clone();
        async move {
            transactions
                .begin()
                .await
                .expect("beginning should succeed")
                .finish()
                .await
                .expect("finishing should succeed");
        }
    });
    // and: the first is finished a moment later
    tokio::time::sleep(A_MOMENT).await;
    let waited = !waiting.is_finished();
    first.finish().await.expect("finishing should succeed");

    // then: the second waited for it
    assert!(waited, "writers are serialised, as SQLite's are");
    // and: began once it was over
    tokio::time::timeout(A_MOMENT * 20, waiting)
        .await
        .expect("the second begins once the first is finished")
        .expect("the second should not panic");
}

#[tokio::test]
async fn each_feature_writes_apart_from_the_others() {
    // given: a transaction begun by one feature
    let mine = InMemoryTransactions::new();
    let theirs = InMemoryTransactions::new();
    let _held = mine.begin().await.expect("beginning should succeed");

    // when: another feature begins one
    let begun = tokio::time::timeout(A_MOMENT * 20, theirs.begin()).await;

    // then: it begins at once
    assert!(
        begun.is_ok(),
        "a transaction of one feature does not hold up another's"
    );
}

#[tokio::test]
async fn a_transaction_whose_commit_is_refused_is_rolled_back_and_runs_nothing_that_follows() {
    // given: a transaction that wrote and asked for a follow-up, whose commit will be refused
    let transactions = InMemoryTransactions::refusing_commits();
    let written = Arc::new(Mutex::new(vec!["warp"]));
    let followed = Arc::new(Mutex::new(false));
    let mut transaction = transactions
        .begin()
        .await
        .expect("beginning should succeed");
    let undoing = written.clone();
    memory::undo_on_rollback(&mut transaction, move || {
        undoing.lock().expect("nothing panics holding it").clear();
    })
    .expect("a transaction begun in memory takes an undo");
    let following = followed.clone();
    transaction.after_commit(async move {
        *following.lock().expect("nothing panics holding it") = true;
    });

    // when: it is finished
    let finished = transaction.finish().await;

    // then: finishing fails
    assert!(finished.is_err());
    // and: what it wrote is undone
    assert!(
        written
            .lock()
            .expect("nothing panics holding it")
            .is_empty(),
        "a commit refused leaves nothing behind"
    );
    // and: the follow-up never ran
    assert!(!*followed.lock().expect("nothing panics holding it"));
}
