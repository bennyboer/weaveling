#![cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]

use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{FakePublisher, Message, MessageId, RoutingKey};
use time::{Duration, OffsetDateTime};

use crate::outbox::{CLAIM_FOR, Delivered, KEPT_FOR, Origin, Outbox};

pub enum Then {
    Commit,
    RollBack,
}

#[async_trait]
pub trait Workbench: Sized {
    type Outbox: Outbox;

    async fn setup(publisher: Arc<FakePublisher>, clock: Arc<FixedClock>) -> Self;

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
    Origin::unversioned("scene_1", "scene")
}

pub struct Bench<W> {
    pub workbench: W,
    pub publisher: Arc<FakePublisher>,
    pub clock: Arc<FixedClock>,
}

impl<W: Workbench> Bench<W> {
    pub async fn setup() -> Self {
        let publisher = Arc::new(FakePublisher::default());
        let clock = Arc::new(FixedClock::new(at(2_000)));
        let workbench = W::setup(publisher.clone(), clock.clone()).await;

        Self {
            workbench,
            publisher,
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

    pub fn ids_heard(&self) -> Vec<MessageId> {
        self.publisher
            .heard()
            .iter()
            .map(|message| message.id)
            .collect()
    }

    pub async fn cleanup(self) {
        self.workbench.cleanup().await;
    }
}

fn delivered(published: usize, refused: usize) -> Delivered {
    Delivered { published, refused }
}

pub async fn the_relay_publishes_what_is_waiting_and_marks_it<W: Workbench>() {
    // given: a message waiting
    let bench = Bench::<W>::setup().await;
    let written = bench.waiting(1).await;

    // then: the relay publishes it
    assert_eq!(bench.deliver(10).await, delivered(1, 0));

    // and: with the id it was written with
    assert_eq!(
        bench.ids_heard(),
        written,
        "the relay publishes the message it was handed, id and all"
    );
    // and: marks it published
    assert_eq!(
        bench.unpublished().await,
        0,
        "a published entry is marked so it is never sent twice"
    );
    bench.cleanup().await;
}

pub async fn the_relay_leaves_nothing_to_do_the_second_time<W: Workbench>() {
    // given: a message the relay published
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench.deliver(10).await;

    // then: the relay finds nothing more to do
    assert_eq!(bench.deliver(10).await, delivered(0, 0));

    // and: publishes nothing more
    assert_eq!(bench.ids_heard().len(), 1);
    bench.cleanup().await;
}

pub async fn a_message_the_broker_refuses_stays_waiting<W: Workbench>() {
    // given: a message waiting
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    // and: a broker refusing it
    bench.publisher.refusing(true);

    // then: the relay counts it refused
    assert_eq!(
        bench.deliver(10).await,
        delivered(0, 1),
        "a refused publish is not a store failure"
    );

    // and: it stays waiting
    assert_eq!(
        bench.unpublished().await,
        1,
        "what the broker would not take must still be waiting"
    );
    bench.cleanup().await;
}

pub async fn a_retried_message_keeps_the_id_it_was_written_with<W: Workbench>() {
    // given: a message the broker refused
    let bench = Bench::<W>::setup().await;
    let written = bench.waiting(1).await;
    bench.publisher.refusing(true);
    bench.deliver(10).await;

    // when: it is delivered once the broker takes it and its claim ran out
    bench.publisher.refusing(false);
    bench
        .clock
        .set(at(2_000) + CLAIM_FOR + Duration::seconds(1));
    bench.deliver(10).await;

    // then: it keeps the id it was written with
    assert_eq!(
        bench.ids_heard(),
        written,
        "a retry must not mint a new id, or the far end cannot recognise the redelivery"
    );
    bench.cleanup().await;
}

pub async fn a_claimed_message_is_left_alone_until_the_claim_runs_out<W: Workbench>() {
    // given: a message the broker refused, claimed by the relay that tried
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench.publisher.refusing(true);
    bench.deliver(10).await;
    // and: a broker that takes it now
    bench.publisher.refusing(false);

    // then: it is left alone while the claim holds
    assert_eq!(
        bench.deliver(10).await,
        delivered(0, 0),
        "another relay must not pick up an entry still claimed"
    );

    // when: the claim runs out
    bench
        .clock
        .set(at(2_000) + CLAIM_FOR + Duration::seconds(1));
    // then: it is published
    assert_eq!(
        bench.deliver(10).await,
        delivered(1, 0),
        "an expired claim is what stops a crashed relay from stranding a message"
    );
    bench.cleanup().await;
}

pub async fn the_relay_takes_no_more_than_it_was_asked_for<W: Workbench>() {
    // given: three messages waiting
    let bench = Bench::<W>::setup().await;
    bench.waiting(3).await;

    // then: the relay asked for two publishes two
    assert_eq!(bench.deliver(2).await, delivered(2, 0));

    // and: one stays waiting
    assert_eq!(bench.unpublished().await, 1);
    bench.cleanup().await;
}

pub async fn an_entry_published_longer_ago_than_we_keep_them_is_deleted<W: Workbench>() {
    // given: a message published longer ago than entries are kept
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench
        .workbench
        .all_published_at(at(2_000) - KEPT_FOR - Duration::days(1))
        .await;

    // then: it is deleted
    assert_eq!(bench.delete_published(at(2_000) - KEPT_FOR, 100).await, 1);

    // and: no entry is left
    assert_eq!(bench.entries().await, 0);
    bench.cleanup().await;
}

pub async fn an_entry_written_after_a_sweep_is_published_once<W: Workbench>() {
    // given: two messages published longer ago than entries are kept
    let bench = Bench::<W>::setup().await;
    bench.waiting(2).await;
    bench.deliver(10).await;
    bench
        .workbench
        .all_published_at(at(2_000) - KEPT_FOR - Duration::days(1))
        .await;
    // and: one of them deleted
    assert_eq!(bench.delete_published(at(2_000) - KEPT_FOR, 1).await, 1);

    // when: another is written
    bench.waiting(1).await;

    // then: the relay publishes it
    assert_eq!(bench.deliver(10).await, delivered(1, 0));
    // and: only once
    assert_eq!(
        bench.deliver(10).await,
        delivered(0, 0),
        "an entry written once others were swept away is numbered anew, so marking it published marks it"
    );
    bench.cleanup().await;
}

pub async fn an_entry_published_recently_is_kept<W: Workbench>() {
    // given: a message published recently
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;
    bench
        .workbench
        .all_published_at(at(2_000) - Duration::days(1))
        .await;

    // then: it is not deleted
    assert_eq!(bench.delete_published(at(2_000) - KEPT_FOR, 100).await, 0);

    // and: its entry stays
    assert_eq!(bench.entries().await, 1);
    bench.cleanup().await;
}

pub async fn an_entry_never_published_is_kept_however_old_it_is<W: Workbench>() {
    // given: a message never published
    let bench = Bench::<W>::setup().await;
    bench.waiting(1).await;

    // then: deleting even years later takes nothing
    assert_eq!(
        bench
            .delete_published(at(2_000) + Duration::days(3_650), 100)
            .await,
        0
    );

    // and: it stays waiting
    assert_eq!(
        bench.unpublished().await,
        1,
        "an old unpublished entry is a stuck message, not rubbish; deleting it would lose it silently"
    );
    bench.cleanup().await;
}

pub async fn deleting_takes_no_more_than_it_was_asked_for<W: Workbench>() {
    // given: three messages published longer ago than entries are kept
    let bench = Bench::<W>::setup().await;
    bench.waiting(3).await;
    bench
        .workbench
        .all_published_at(at(2_000) - KEPT_FOR - Duration::days(1))
        .await;

    // then: deleting asked for two takes two
    assert_eq!(bench.delete_published(at(2_000) - KEPT_FOR, 2).await, 2);

    // and: one entry stays
    assert_eq!(bench.entries().await, 1);
    bench.cleanup().await;
}

pub async fn messages_are_published_in_the_order_they_were_written<W: Workbench>() {
    // given: five messages waiting
    let bench = Bench::<W>::setup().await;
    let written = bench.waiting(5).await;

    // when: the relay runs
    bench.deliver(10).await;

    // then: they are published in the order they were written
    assert_eq!(
        bench.ids_heard(),
        written,
        "a detach overtaking the attach it undoes would leave a read model wrong for good"
    );
    bench.cleanup().await;
}

pub async fn a_message_enqueued_beside_a_write_is_relayed_whole_once_that_write_commits<
    W: Workbench,
>() {
    let bench = Bench::<W>::setup().await;
    // given: a message
    let message = a_link_message();

    // when: it is enqueued beside a write that commits
    bench
        .workbench
        .enqueue(std::slice::from_ref(&message), Then::Commit)
        .await;
    // and: the relay runs
    bench.deliver(10).await;

    // then: it is published whole
    assert_eq!(
        bench.publisher.heard(),
        [message],
        "a writer that is not event-sourced gets the same relay, id and all"
    );
    bench.cleanup().await;
}

pub async fn a_message_enqueued_beside_a_write_that_rolls_back_goes_with_it<W: Workbench>() {
    let bench = Bench::<W>::setup().await;

    // when: a message is enqueued beside a write that rolls back
    bench
        .workbench
        .enqueue(&[a_link_message()], Then::RollBack)
        .await;

    // then: no entry is left
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
            an_entry_written_after_a_sweep_is_published_once,
            an_entry_never_published_is_kept_however_old_it_is,
            deleting_takes_no_more_than_it_was_asked_for,
            messages_are_published_in_the_order_they_were_written,
            a_message_enqueued_beside_a_write_is_relayed_whole_once_that_write_commits,
            a_message_enqueued_beside_a_write_that_rolls_back_goes_with_it,
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
