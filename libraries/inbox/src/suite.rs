#![cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]

use std::sync::Arc;

use clock::FixedClock;
use messaging::{ListenerName, MessageId};
use time::{Duration, OffsetDateTime};
use tokio::sync::oneshot;
use transacting::{Transaction, Transactions, Tried};

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

    pub async fn heard_before(&self, listener: &str, message: MessageId) -> bool {
        let mut transaction = self.begun().await;
        let before = self
            .workbench
            .inbox()
            .heard_before(&mut transaction, &a_listener(listener), message)
            .await
            .expect("hearing should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        before
    }

    pub async fn was_heard(&self, listener: &str, message: MessageId) -> bool {
        let mut transaction = self.begun().await;
        let heard = self
            .workbench
            .inbox()
            .was_heard(&mut transaction, &a_listener(listener), message)
            .await
            .expect("asking should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        heard
    }

    pub async fn heard_at(&self, seconds: i64, listener: &str, message: MessageId) {
        self.clock.set(at(seconds));
        assert!(
            !self.heard_before(listener, message).await,
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

pub async fn asking_whether_a_message_was_heard_hears_nothing<W: Workbench>() {
    // given: a message no listener heard
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);

    // when: whether it was heard is asked twice
    let before = bench.was_heard("mail-issued-invitation", message).await;
    let still = bench.was_heard("mail-issued-invitation", message).await;
    // and: it is heard after
    bench.heard_at(5, "mail-issued-invitation", message).await;

    // then: asking heard nothing
    assert_eq!(
        (before, still),
        (false, false),
        "asking is only asking, and leaves the message to be heard"
    );
    // and: the hearing did
    assert!(bench.was_heard("mail-issued-invitation", message).await);
    // and: only for the listener that heard it
    assert!(
        !bench.was_heard("another-listener", message).await,
        "each listener for itself"
    );
    bench.cleanup().await;
}

pub async fn a_message_was_heard_before_from_its_second_hearing_on<W: Workbench>() {
    // given: a message and a listener
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    let mut transaction = bench.begun().await;
    let listener = a_listener("note-role-changed");
    let inbox = bench.workbench.inbox();

    // when: the listener hears it twice in one transaction
    let first = inbox
        .heard_before(&mut transaction, &listener, message)
        .await
        .expect("hearing should succeed");
    let again = inbox
        .heard_before(&mut transaction, &listener, message)
        .await
        .expect("hearing should succeed");
    // and: the transaction is finished
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: it was heard before only the second time
    assert_eq!((first, again), (false, true));
    // and: it stays heard
    assert!(
        bench.heard_before("note-role-changed", message).await,
        "what was heard stays heard once its transaction commits"
    );
    bench.cleanup().await;
}

pub async fn a_message_heard_in_a_dropped_transaction_was_never_heard<W: Workbench>() {
    // given: a message heard in a transaction
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    let mut transaction = bench.begun().await;
    bench
        .workbench
        .inbox()
        .heard_before(&mut transaction, &a_listener("note-role-changed"), message)
        .await
        .expect("hearing should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: it was never heard
    assert!(
        !bench.heard_before("note-role-changed", message).await,
        "a handler that failed hears its message anew when it is redelivered"
    );
    bench.cleanup().await;
}

pub async fn each_listener_hears_a_message_for_itself<W: Workbench>() {
    // given: a message no listener heard
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);

    // when: two listeners hear it
    let heard = (
        bench.heard_before("note-role-changed", message).await,
        bench.heard_before("catalogue-members", message).await,
    );

    // then: neither heard it before
    assert_eq!(heard, (false, false));
    bench.cleanup().await;
}

pub async fn each_message_is_heard_on_its_own<W: Workbench>() {
    // given: an inbox that heard nothing
    let bench = Bench::<W>::setup().await;

    // when: one listener hears two messages
    let heard = (
        bench.heard_before("note-role-changed", a_message(1)).await,
        bench.heard_before("note-role-changed", a_message(2)).await,
    );

    // then: neither was heard before
    assert_eq!(heard, (false, false));
    bench.cleanup().await;
}

pub async fn what_was_heard_long_ago_is_forgotten_oldest_first<W: Workbench>() {
    // given: eight messages heard long ago in no order
    let bench = Bench::<W>::setup().await;
    let messages: Vec<MessageId> = (1..=8).map(a_message).collect();
    let heard_in_no_order = [7, 2, 5, 1, 8, 3, 6, 4];
    for nth in heard_in_no_order {
        bench
            .heard_at(nth * 10, "note-role-changed", messages[nth as usize - 1])
            .await;
    }
    // and: one heard at the moment
    let latest = a_message(9);
    bench.heard_at(100, "note-role-changed", latest).await;

    // when: at most four heard before that moment are forgotten
    let forgotten = bench.forget_before(100, 4).await;
    bench.clock.set(at(200));

    // then: four are forgotten
    assert_eq!(forgotten, 4, "no more than asked for");
    // and: those heard longest ago
    let mut heard = Vec::new();
    for message in messages {
        heard.push(bench.heard_before("note-role-changed", message).await);
    }
    assert_eq!(
        heard,
        [false, false, false, false, true, true, true, true],
        "the four heard longest ago go first"
    );
    // and: the one heard at the moment stays heard
    assert!(bench.heard_before("note-role-changed", latest).await);
    bench.cleanup().await;
}

pub async fn what_was_heard_at_the_moment_itself_is_kept<W: Workbench>() {
    // given: a message heard at a moment
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    bench.heard_at(10, "note-role-changed", message).await;

    // when: what was heard before that moment is forgotten, and then what was heard before the next
    let forgotten = (
        bench.forget_before(10, 100).await,
        bench.forget_before(11, 100).await,
    );

    // then: it is kept the first time and forgotten the second
    assert_eq!(forgotten, (0, 1));
    bench.cleanup().await;
}

pub async fn a_dropped_forgetting_keeps_what_was_heard<W: Workbench>() {
    // given: a message heard long ago
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    bench.heard_at(10, "note-role-changed", message).await;
    // and: a transaction forgetting it
    let mut transaction = bench.begun().await;
    bench
        .workbench
        .inbox()
        .forget_heard_before(&mut transaction, at(20), 100)
        .await
        .expect("forgetting should succeed");

    // when: that transaction is dropped
    drop(transaction);

    // then: the message stays heard
    assert!(bench.heard_before("note-role-changed", message).await);
    bench.cleanup().await;
}

pub async fn two_handlers_hearing_one_message_at_once_hear_it_once<W: Workbench>() {
    // given: a message and a listener
    let bench = Bench::<W>::setup().await;
    let message = a_message(1);
    let listener = a_listener("note-role-changed");
    let inbox = bench.workbench.inbox();
    let (heard, hearing) = oneshot::channel();

    // when: two handlers hear it at once
    let (first, second) = tokio::join!(
        async {
            let mut transaction = bench.begun().await;
            let first = inbox
                .heard_before(&mut transaction, &listener, message)
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

            transacting::retrying!(bench.workbench.transactions(), |transaction| {
                inbox
                    .heard_before(transaction, &listener, message)
                    .await
                    .map_err(Tried::Refused)
            })
            .expect("hearing should succeed")
        },
    );

    // then: only one hears it new
    assert_eq!(
        (first, second),
        (false, true),
        "the second waits for the first, or loses to it and hears again, and then finds the message heard"
    );
    bench.cleanup().await;
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::conformance_tests!($workbench:
            a_message_was_heard_before_from_its_second_hearing_on,
            a_message_heard_in_a_dropped_transaction_was_never_heard,
            each_listener_hears_a_message_for_itself,
            each_message_is_heard_on_its_own,
            what_was_heard_long_ago_is_forgotten_oldest_first,
            what_was_heard_at_the_moment_itself_is_kept,
            a_dropped_forgetting_keeps_what_was_heard,
            two_handlers_hearing_one_message_at_once_hear_it_once,
            asking_whether_a_message_was_heard_hears_nothing,
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
