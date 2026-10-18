#![cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{Message, MessageId, Publisher, RoutingKey, Undelivered};
use time::{Duration, OffsetDateTime};

use crate::outbox::{CLAIM_FOR, Delivered, KEPT_FOR, Origin, Outbox};

#[derive(Default)]
pub struct Switchboard {
    heard: Mutex<Vec<Message>>,
    refusing: AtomicBool,
}

#[async_trait]
impl Publisher for Switchboard {
    async fn publish(&self, message: Message) -> Result<(), Undelivered> {
        if self.refusing.load(Ordering::SeqCst) {
            return Err(Undelivered {
                routing: message.routing,
                because: "the broker is not listening".into(),
            });
        }
        self.heard
            .lock()
            .expect("the switchboard is never poisoned")
            .push(message);

        Ok(())
    }
}

impl Switchboard {
    pub fn refusing(&self, refusing: bool) {
        self.refusing.store(refusing, Ordering::SeqCst);
    }

    pub fn heard(&self) -> Vec<Message> {
        self.heard
            .lock()
            .expect("the switchboard is never poisoned")
            .clone()
    }

    pub fn ids_heard(&self) -> Vec<MessageId> {
        self.heard().iter().map(|message| message.id).collect()
    }
}

pub enum Then {
    Commit,
    RollBack,
}

#[async_trait]
pub trait Workbench: Sized {
    type Outbox: Outbox;

    async fn setup(publisher: Arc<Switchboard>, clock: Arc<FixedClock>) -> Self;

    fn outbox(&self) -> &Self::Outbox;

    async fn enqueue(&self, messages: &[Message], then: Then);

    async fn published(&self) -> Vec<bool>;

    async fn all_published_at(&self, when: OffsetDateTime);

    async fn cleanup(self);
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub fn a_link_message() -> Message {
    Message::opening(
        RoutingKey::parse("scene.idea.linked").expect("a plain key is fine"),
        serde_json::json!({ "scene": "scene_1", "idea": "idea_1" }),
        at(2_000),
    )
}

pub fn from_a_scene() -> Origin<'static> {
    Origin {
        aggregate: "scene_1",
        kind: "scene",
        version: 0,
    }
}

pub struct Bench<W> {
    pub workbench: W,
    pub heard: Arc<Switchboard>,
    pub clock: Arc<FixedClock>,
}

impl<W: Workbench> Bench<W> {
    pub async fn setup() -> Self {
        let heard = Arc::new(Switchboard::default());
        let clock = Arc::new(FixedClock::new(at(2_000)));
        let workbench = W::setup(heard.clone(), clock.clone()).await;

        Self {
            workbench,
            heard,
            clock,
        }
    }

    pub async fn waiting(&self, how_many: usize) -> Vec<MessageId> {
        let messages: Vec<Message> = (0..how_many).map(|_| a_link_message()).collect();
        self.workbench.enqueue(&messages, Then::Commit).await;

        messages.iter().map(|message| message.id).collect()
    }

    pub async fn deliver(&self, at_most: i64) -> Delivered {
        self.workbench
            .outbox()
            .deliver(at_most)
            .await
            .expect("delivering should succeed")
    }

    pub async fn delete_published(&self, before: OffsetDateTime, at_most: i64) -> u64 {
        self.workbench
            .outbox()
            .delete_published(before, at_most)
            .await
            .expect("deleting should succeed")
    }

    pub async fn unpublished(&self) -> usize {
        self.workbench
            .published()
            .await
            .iter()
            .filter(|published| !**published)
            .count()
    }

    pub async fn entries(&self) -> usize {
        self.workbench.published().await.len()
    }

    pub async fn cleanup(self) {
        self.workbench.cleanup().await;
    }
}

fn delivered(published: usize, refused: usize) -> Delivered {
    Delivered { published, refused }
}

pub async fn the_relay_publishes_what_is_waiting_and_marks_it<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let written = bench.waiting(1).await;

    assert_eq!(bench.deliver(10).await, delivered(1, 0));

    assert_eq!(
        bench.heard.ids_heard(),
        written,
        "the relay publishes the message it was handed, id and all"
    );
    assert_eq!(
        bench.unpublished().await,
        0,
        "a published entry is marked so it is never sent twice"
    );
    bench.cleanup().await;
}

pub async fn the_relay_leaves_nothing_to_do_the_second_time<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench.deliver(10).await;

    assert_eq!(bench.deliver(10).await, delivered(0, 0));

    assert_eq!(bench.heard.ids_heard().len(), 1);
    bench.cleanup().await;
}

pub async fn a_message_the_broker_refuses_stays_waiting<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench.heard.refusing(true);

    assert_eq!(
        bench.deliver(10).await,
        delivered(0, 1),
        "a refused publish is not a store failure"
    );

    assert_eq!(
        bench.unpublished().await,
        1,
        "what the broker would not take must still be waiting"
    );
    bench.cleanup().await;
}

pub async fn a_retried_message_keeps_the_id_it_was_written_with<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let written = bench.waiting(1).await;
    bench.heard.refusing(true);
    bench.deliver(10).await;

    bench.heard.refusing(false);
    bench
        .clock
        .set(at(2_000) + CLAIM_FOR + Duration::seconds(1));
    bench.deliver(10).await;

    assert_eq!(
        bench.heard.ids_heard(),
        written,
        "a retry must not mint a new id, or the far end cannot recognise the redelivery"
    );
    bench.cleanup().await;
}

pub async fn a_claimed_message_is_left_alone_until_the_claim_runs_out<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench.heard.refusing(true);
    bench.deliver(10).await;
    bench.heard.refusing(false);

    assert_eq!(
        bench.deliver(10).await,
        delivered(0, 0),
        "another relay must not pick up an entry still claimed"
    );

    bench
        .clock
        .set(at(2_000) + CLAIM_FOR + Duration::seconds(1));
    assert_eq!(
        bench.deliver(10).await,
        delivered(1, 0),
        "an expired claim is what stops a crashed relay from stranding a message"
    );
    bench.cleanup().await;
}

pub async fn the_relay_takes_no_more_than_it_was_asked_for<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(3).await;

    assert_eq!(bench.deliver(2).await, delivered(2, 0));

    assert_eq!(bench.unpublished().await, 1);
    bench.cleanup().await;
}

pub async fn an_entry_published_longer_ago_than_we_keep_them_is_deleted<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench
        .workbench
        .all_published_at(at(2_000) - KEPT_FOR - Duration::days(1))
        .await;

    assert_eq!(bench.delete_published(at(2_000) - KEPT_FOR, 100).await, 1);

    assert_eq!(bench.entries().await, 0);
    bench.cleanup().await;
}

pub async fn an_entry_published_recently_is_kept<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench
        .workbench
        .all_published_at(at(2_000) - Duration::days(1))
        .await;

    assert_eq!(bench.delete_published(at(2_000) - KEPT_FOR, 100).await, 0);

    assert_eq!(bench.entries().await, 1);
    bench.cleanup().await;
}

pub async fn an_entry_never_published_is_kept_however_old_it_is<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;

    assert_eq!(
        bench
            .delete_published(at(2_000) + Duration::days(3_650), 100)
            .await,
        0
    );

    assert_eq!(
        bench.unpublished().await,
        1,
        "an old unpublished entry is a stuck message, not rubbish; deleting it would lose it silently"
    );
    bench.cleanup().await;
}

pub async fn deleting_takes_no_more_than_it_was_asked_for<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    bench.waiting(3).await;
    bench
        .workbench
        .all_published_at(at(2_000) - KEPT_FOR - Duration::days(1))
        .await;

    assert_eq!(bench.delete_published(at(2_000) - KEPT_FOR, 2).await, 2);

    assert_eq!(bench.entries().await, 1);
    bench.cleanup().await;
}

pub async fn messages_are_published_in_the_order_they_were_written<W: Workbench>() {
    let bench = Bench::<W>::setup().await;
    let written = bench.waiting(5).await;

    bench.deliver(10).await;

    assert_eq!(
        bench.heard.ids_heard(),
        written,
        "a detach overtaking the attach it undoes would leave a read model wrong for good"
    );
    bench.cleanup().await;
}

pub async fn a_message_enqueued_beside_a_write_is_relayed_whole_once_that_write_commits<
    W: Workbench,
>() {
    let bench = Bench::<W>::setup().await;
    let message = a_link_message();

    bench
        .workbench
        .enqueue(std::slice::from_ref(&message), Then::Commit)
        .await;
    bench.deliver(10).await;

    assert_eq!(
        bench.heard.heard(),
        [message],
        "a writer that is not event-sourced gets the same relay, id and all"
    );
    bench.cleanup().await;
}

pub async fn a_message_enqueued_beside_a_write_that_rolls_back_goes_with_it<W: Workbench>() {
    let bench = Bench::<W>::setup().await;

    bench
        .workbench
        .enqueue(&[a_link_message()], Then::RollBack)
        .await;

    assert_eq!(
        bench.entries().await,
        0,
        "the whole point of enqueueing in the writer's transaction: a change that never happened is never announced"
    );
    bench.cleanup().await;
}

#[macro_export]
macro_rules! outbox_conformance_tests {
    ($workbench:ty) => {
        $crate::outbox_conformance_tests!($workbench:
            the_relay_publishes_what_is_waiting_and_marks_it,
            the_relay_leaves_nothing_to_do_the_second_time,
            a_message_the_broker_refuses_stays_waiting,
            a_retried_message_keeps_the_id_it_was_written_with,
            a_claimed_message_is_left_alone_until_the_claim_runs_out,
            the_relay_takes_no_more_than_it_was_asked_for,
            an_entry_published_longer_ago_than_we_keep_them_is_deleted,
            an_entry_published_recently_is_kept,
            an_entry_never_published_is_kept_however_old_it_is,
            deleting_takes_no_more_than_it_was_asked_for,
            messages_are_published_in_the_order_they_were_written,
            a_message_enqueued_beside_a_write_is_relayed_whole_once_that_write_commits,
        );
    };
    ($workbench:ty, in_transactions) => {
        $crate::outbox_conformance_tests!($workbench);
        $crate::outbox_conformance_tests!($workbench: a_message_enqueued_beside_a_write_that_rolls_back_goes_with_it);
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
