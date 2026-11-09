use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::retrying::TRIES;
use crate::{Failure, InMemoryTransactions, LostRace, Transaction, Transactions, memory};

#[derive(Debug, PartialEq)]
enum Refusal {
    Lost,
    Other,
    Failed,
}

impl LostRace for Refusal {
    fn lost_race(&self) -> bool {
        matches!(self, Refusal::Lost)
    }
}

impl From<Failure> for Refusal {
    fn from(_: Failure) -> Self {
        Refusal::Failed
    }
}

#[derive(Clone, Default)]
struct Words(Arc<Mutex<Vec<String>>>);

impl Words {
    fn write(&self, transaction: &mut Transaction, word: String) {
        self.0.lock().expect("nothing panics holding it").push(word);
        let words = self.0.clone();
        memory::undo_on_rollback(transaction, move || {
            words.lock().expect("nothing panics holding it").pop();
        })
        .expect("a transaction begun in memory takes an undo");
    }

    fn written(&self) -> Vec<String> {
        self.0.lock().expect("nothing panics holding it").clone()
    }
}

#[tokio::test]
async fn a_try_that_wins_is_finished_and_answered_at_once() {
    // when: a write wins its first try
    let words = Words::default();
    let mut tried = 0;

    let answered: Result<_, Refusal> =
        crate::retrying!(&InMemoryTransactions::new(), |transaction| {
            tried += 1;
            words.write(transaction, "warp".to_owned());
            Ok("woven")
        });

    // then: it is finished and answered after that one try
    assert_eq!(
        (answered, tried, words.written()),
        (Ok("woven"), 1, vec!["warp".to_owned()])
    );
}

#[tokio::test]
async fn a_lost_race_is_tried_again_in_a_fresh_transaction() {
    // when: a write loses a race twice and then wins
    let words = Words::default();
    let mut tried = 0;

    let answered: Result<_, Refusal> =
        crate::retrying!(&InMemoryTransactions::new(), |transaction| {
            tried += 1;
            words.write(transaction, format!("try {tried}"));
            if tried < 3 {
                return Err(Refusal::Lost);
            }
            Ok(tried)
        });

    // then: the third try is the answer
    assert_eq!(answered, Ok(3));
    // and: only what the third try wrote is kept
    assert_eq!(
        words.written(),
        vec!["try 3".to_owned()],
        "what a lost try wrote is rolled back before the next begins"
    );
}

#[tokio::test]
async fn after_five_lost_races_the_last_loss_is_the_answer() {
    // when: a write loses every race
    let words = Words::default();
    let mut tried = 0;

    let answered: Result<(), Refusal> =
        crate::retrying!(&InMemoryTransactions::new(), |transaction| {
            tried += 1;
            words.write(transaction, format!("try {tried}"));
            Err(Refusal::Lost)
        });

    // then: after its last try its last loss is the answer
    assert_eq!((answered, tried), (Err(Refusal::Lost), TRIES));
    // and: nothing it wrote is kept
    assert!(words.written().is_empty());
}

#[tokio::test]
async fn a_refusal_other_than_a_lost_race_is_answered_without_trying_again() {
    // when: a write is refused other than by losing a race
    let words = Words::default();
    let mut tried = 0;

    let answered: Result<(), Refusal> =
        crate::retrying!(&InMemoryTransactions::new(), |transaction| {
            tried += 1;
            words.write(transaction, "warp".to_owned());
            Err(Refusal::Other)
        });

    // then: the refusal is answered after one try
    assert_eq!((answered, tried), (Err(Refusal::Other), 1));
    // and: nothing it wrote is kept
    assert!(words.written().is_empty());
}

#[tokio::test]
async fn a_write_given_fewer_tries_gives_up_after_them() {
    // when: a write given two tries loses every race
    let words = Words::default();
    let mut tried = 0;

    let answered: Result<(), Refusal> =
        crate::retrying!(&InMemoryTransactions::new(), 2, |transaction| {
            tried += 1;
            words.write(transaction, format!("try {tried}"));
            Err(Refusal::Lost)
        });

    // then: it gives up after those two
    assert_eq!((answered, tried), (Err(Refusal::Lost), 2));
}

#[tokio::test]
async fn a_write_given_one_try_answers_a_lost_race_at_once() {
    // when: a write given one try loses a race
    let words = Words::default();
    let mut tried = 0;

    let answered: Result<(), Refusal> =
        crate::retrying!(&InMemoryTransactions::new(), 1, |transaction| {
            tried += 1;
            words.write(transaction, "warp".to_owned());
            Err(Refusal::Lost)
        });

    // then: the loss is answered at once
    assert_eq!(
        (answered, tried),
        (Err(Refusal::Lost), 1),
        "a write aimed at a version hears it moved on rather than having it papered over"
    );
    // and: nothing it wrote is kept
    assert!(words.written().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retried_write_runs_on_any_task() {
    // when: a retried write runs on a spawned task
    let transactions = InMemoryTransactions::new();
    let words = Words::default();

    let spawned = tokio::spawn({
        let words = words.clone();
        async move {
            let answered: Result<_, Refusal> = crate::retrying!(&transactions, |transaction| {
                words.write(transaction, "warp".to_owned());
                tokio::task::yield_now().await;
                Ok(())
            });
            answered
        }
    });

    // then: it is answered
    assert_eq!(spawned.await.expect("the task should not panic"), Ok(()));
    // and: what it wrote is kept
    assert_eq!(words.written(), vec!["warp".to_owned()]);
}

#[test]
fn each_pause_is_drawn_up_to_twice_the_one_before_starting_at_ten_milliseconds() {
    // expect: each pause drawn at random, up to twice the one before, from ten milliseconds
    for (tried, longest) in [(1, 10), (2, 20), (3, 40), (4, 80)] {
        let longest = Duration::from_millis(longest);
        let drawn: Vec<Duration> = (0..200)
            .map(|_| crate::retrying::pause_after(tried))
            .collect();

        assert!(
            drawn.iter().all(|pause| *pause <= longest),
            "after try {tried} no pause is longer than {longest:?}"
        );
        assert!(
            drawn.iter().any(|pause| *pause > longest / 2),
            "after try {tried} pauses reach up towards {longest:?}"
        );
        assert!(
            drawn.iter().collect::<HashSet<_>>().len() > 100,
            "pauses are drawn at random, so writers that collided do not collide again"
        );
    }
}

#[tokio::test]
async fn joining_a_transaction_handed_in_leaves_its_lost_race_to_the_owner() {
    // given: a transaction handed in
    let transactions = InMemoryTransactions::new();
    let words = Words::default();
    let mut tried = 0;
    let mut owned = transactions
        .begin()
        .await
        .expect("beginning should succeed");

    // when: a write joining it loses a race
    let answered: Result<(), Refusal> =
        crate::joining!(&transactions, Some(&mut owned), |transaction| {
            tried += 1;
            words.write(transaction, "warp".to_owned());
            Err(Refusal::Lost)
        });

    // then: the loss is answered after one try
    assert_eq!((answered, tried), (Err(Refusal::Lost), 1));
    // and: what it wrote is left to the owner
    assert_eq!(
        words.written(),
        vec!["warp".to_owned()],
        "the owner decides whether what was written in its transaction stays"
    );
}

#[tokio::test]
async fn joining_no_transaction_tries_in_its_own_until_it_wins() {
    // when: a write joining no transaction loses a race and then wins
    let words = Words::default();
    let mut tried = 0;

    let answered: Result<_, Refusal> =
        crate::joining!(&InMemoryTransactions::new(), None, |transaction| {
            tried += 1;
            words.write(transaction, format!("try {tried}"));
            if tried < 2 {
                return Err(Refusal::Lost);
            }
            Ok(tried)
        });

    // then: the second try is the answer
    assert_eq!(answered, Ok(2));
    // and: only what the second try wrote is kept
    assert_eq!(words.written(), vec!["try 2".to_owned()]);
}
