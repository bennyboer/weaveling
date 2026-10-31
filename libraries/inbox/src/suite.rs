#![cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]

use std::sync::Arc;

use clock::FixedClock;
use messaging::{ListenerName, MessageId};
use time::{Duration, OffsetDateTime};
use tokio::sync::oneshot;
use transacting::{Transaction, Transactions};

use crate::Inbox;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Inbox: Inbox;

    async fn setup(clock: Arc<FixedClock>) -> Self;

    fn inbox(&self) -> &Self::Inbox;

    fn transactions(&self) -> &dyn Transactions;

    async fn cleanup(self);
}

pub struct Bench<W> {
    pub workbench: W,
    pub clock: Arc<FixedClock>,
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_700_000_000 + seconds)
}

pub fn a_message(seconds: i64) -> MessageId {
    MessageId::generate(at(seconds))
}

pub fn a_listener(name: &str) -> ListenerName {
    ListenerName::parse(name).expect("a plain name is fine")
}

impl<W: Workbench> Bench<W> {
    pub async fn setup() -> Self {
        let clock = Arc::new(FixedClock::new(at(0)));

        Self {
            workbench: W::setup(clock.clone()).await,
            clock,
        }
    }

    pub async fn begun(&self) -> Transaction {
        self.workbench
            .transactions()
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub async fn hears(&self, listener: &str, message: MessageId) -> bool {
        let mut transaction = self.begun().await;
        let first = self
            .workbench
            .inbox()
            .first_time(&mut transaction, &a_listener(listener), message)
            .await
            .expect("hearing should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        first
    }

    pub async fn heard_at(&self, seconds: i64, listener: &str, message: MessageId) {
        self.clock.set(at(seconds));
        assert!(
            self.hears(listener, message).await,
            "a message not heard before"
        );
    }

    pub async fn forget_before(&self, seconds: i64, at_most: u64) -> u64 {
        let mut transaction = self.begun().await;
        let forgotten = self
            .workbench
            .inbox()
            .forget_heard_before(&mut transaction, at(seconds), at_most)
            .await
            .expect("forgetting should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        forgotten
    }

    pub async fn cleanup(self) {
        self.workbench.cleanup().await;
    }
}

pub async fn a_message_is_heard_the_first_time_only<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    let mut transaction = bench.begun().await;
    let listener = a_listener("note-role-changed");
    let inbox = bench.workbench.inbox();

    let first = inbox
        .first_time(&mut transaction, &listener, message)
        .await
        .expect("hearing should succeed");
    let again = inbox
        .first_time(&mut transaction, &listener, message)
        .await
        .expect("hearing should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    assert_eq!((first, again), (true, false));
    assert!(
        !bench.hears("note-role-changed", message).await,
        "what was heard stays heard once its transaction commits"
    );
    bench.cleanup().await;
}

pub async fn a_message_heard_in_a_dropped_transaction_was_never_heard<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    let mut transaction = bench.begun().await;
    bench
        .workbench
        .inbox()
        .first_time(&mut transaction, &a_listener("note-role-changed"), message)
        .await
        .expect("hearing should succeed");

    drop(transaction);

    assert!(
        bench.hears("note-role-changed", message).await,
        "a handler that failed hears its message again when it is redelivered"
    );
    bench.cleanup().await;
}

pub async fn each_listener_hears_a_message_for_itself<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);

    let heard = (
        bench.hears("note-role-changed", message).await,
        bench.hears("catalogue-members", message).await,
    );

    assert_eq!(heard, (true, true));
    bench.cleanup().await;
}

pub async fn each_message_is_heard_on_its_own<W: Workbench>() {
    let bench = Bench::<W>::setup().await;

    let heard = (
        bench.hears("note-role-changed", a_message(1)).await,
        bench.hears("note-role-changed", a_message(2)).await,
    );

    assert_eq!(heard, (true, true));
    bench.cleanup().await;
}

pub async fn what_was_heard_long_ago_is_forgotten_oldest_first<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let messages: Vec<MessageId> = (1..=8).map(a_message).collect();
    let heard_in_no_order = [7, 2, 5, 1, 8, 3, 6, 4];
    for nth in heard_in_no_order {
        bench
            .heard_at(nth * 10, "note-role-changed", messages[nth as usize - 1])
            .await;
    }
    let latest = a_message(9);
    bench.heard_at(100, "note-role-changed", latest).await;

    let forgotten = bench.forget_before(100, 4).await;
    bench.clock.set(at(200));

    assert_eq!(forgotten, 4, "no more than asked for");
    let mut heard_again = Vec::new();
    for message in messages {
        heard_again.push(bench.hears("note-role-changed", message).await);
    }
    assert_eq!(
        heard_again,
        [true, true, true, true, false, false, false, false],
        "the four heard longest ago go first"
    );
    assert!(!bench.hears("note-role-changed", latest).await);
    bench.cleanup().await;
}

pub async fn what_was_heard_at_the_moment_itself_is_kept<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    bench.heard_at(10, "note-role-changed", message).await;

    let forgotten = (
        bench.forget_before(10, 100).await,
        bench.forget_before(11, 100).await,
    );

    assert_eq!(forgotten, (0, 1));
    bench.cleanup().await;
}

pub async fn a_dropped_forgetting_keeps_what_was_heard<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    bench.heard_at(10, "note-role-changed", message).await;
    let mut transaction = bench.begun().await;
    bench
        .workbench
        .inbox()
        .forget_heard_before(&mut transaction, at(20), 100)
        .await
        .expect("forgetting should succeed");

    drop(transaction);

    assert!(!bench.hears("note-role-changed", message).await);
    bench.cleanup().await;
}

pub async fn two_handlers_hearing_one_message_at_once_hear_it_once<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    let listener = a_listener("note-role-changed");
    let inbox = bench.workbench.inbox();
    let (heard, hearing) = oneshot::channel();

    let (first, second) = tokio::join!(
        async {
            let mut transaction = bench.begun().await;
            let first = inbox
                .first_time(&mut transaction, &listener, message)
                .await
                .expect("hearing should succeed");
            heard.send(()).expect("the second handler is waiting");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            transaction
                .finish()
                .await
                .expect("finishing should succeed");

            first
        },
        async {
            hearing.await.expect("the first handler heard the message");
            let mut transaction = bench.begun().await;
            let second = inbox
                .first_time(&mut transaction, &listener, message)
                .await
                .expect("hearing should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");

            second
        },
    );

    assert_eq!(
        (first, second),
        (true, false),
        "the second waits for the first and then finds the message heard"
    );
    bench.cleanup().await;
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::conformance_tests!($workbench:
            a_message_is_heard_the_first_time_only,
            a_message_heard_in_a_dropped_transaction_was_never_heard,
            each_listener_hears_a_message_for_itself,
            each_message_is_heard_on_its_own,
            what_was_heard_long_ago_is_forgotten_oldest_first,
            what_was_heard_at_the_moment_itself_is_kept,
            a_dropped_forgetting_keeps_what_was_heard,
            two_handlers_hearing_one_message_at_once_hear_it_once,
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
