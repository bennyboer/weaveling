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

    drop(transaction);

    assert_eq!(
        *undone.lock().expect("nothing panics holding it"),
        vec![3, 2, 1],
        "a later write may build on an earlier one, so it is undone first"
    );
}

#[tokio::test]
async fn a_second_transaction_begins_only_once_the_first_is_over() {
    let transactions = InMemoryTransactions::new();
    let first = transactions
        .begin()
        .await
        .expect("beginning should succeed");

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
    tokio::time::sleep(A_MOMENT).await;
    let waited = !waiting.is_finished();
    first.finish().await.expect("finishing should succeed");

    assert!(waited, "writers are serialised, as SQLite's are");
    tokio::time::timeout(A_MOMENT * 20, waiting)
        .await
        .expect("the second begins once the first is finished")
        .expect("the second should not panic");
}

#[tokio::test]
async fn each_feature_writes_apart_from_the_others() {
    let mine = InMemoryTransactions::new();
    let theirs = InMemoryTransactions::new();
    let _held = mine.begin().await.expect("beginning should succeed");

    let begun = tokio::time::timeout(A_MOMENT * 20, theirs.begin()).await;

    assert!(
        begun.is_ok(),
        "a transaction of one feature does not hold up another's"
    );
}
