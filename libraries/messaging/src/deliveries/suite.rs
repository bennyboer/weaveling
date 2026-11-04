use time::{Duration, OffsetDateTime};

use crate::delivering;
use crate::delivering::{ATTEMPTS, CLAIM_FOR, Claim, DeadLetter, Deliveries, Queued};
use crate::listening::ListenerName;
use crate::message::Message;
use crate::routing::RoutingKey;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Deliveries;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub fn a_listener(named: &str) -> ListenerName {
    ListenerName::parse(named).expect("a plain name is fine")
}

pub fn a_message(routing: &str) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a plain key is fine"),
        serde_json::json!({ "nothing": "much" }),
        at(1_000),
    )
}

pub async fn an_enqueued_delivery_is_claimed_when_it_is_due(deliveries: &impl Deliveries) {
    // given: a delivery enqueued for a listener
    deliveries
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");

    // when: what is due is claimed
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    // then: it is claimed for that listener, not yet attempted
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].listener, a_listener("catalogue-idea"));
    assert_eq!(claimed[0].attempts, 0);
}

pub async fn one_message_becomes_one_delivery_per_listener(deliveries: &impl Deliveries) {
    // given: one message enqueued for two listeners
    let message = a_message("idea.discarded");
    for listener in ["unpin-discarded-idea", "detach-discarded-idea"] {
        deliveries
            .enqueue(&a_listener(listener), &message)
            .await
            .expect("enqueuing should succeed");
    }

    // when: what is due is claimed
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    // then: there is a delivery for each
    assert_eq!(
        claimed.len(),
        2,
        "each listener has its own queue, so one failing must not redeliver to the other"
    );
}

pub async fn a_handled_delivery_is_gone(deliveries: &impl Deliveries) {
    // given: a claimed delivery
    deliveries
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    // when: it is marked handled
    deliveries
        .mark_as_handled(&claimed[0])
        .await
        .expect("acking should succeed");

    // then: nothing waits
    assert_eq!(deliveries.waiting().await.expect("counting"), 0);
}

pub async fn a_claimed_delivery_is_not_handed_out_twice(deliveries: &impl Deliveries) {
    // given: a claimed delivery
    deliveries
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");
    deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    // when: what is due is claimed again
    let again = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    // then: it is not handed out again
    assert!(
        again.is_empty(),
        "two workers draining at once must not hand one delivery to a listener twice"
    );
}

pub async fn a_refused_delivery_waits_and_counts_the_attempt(deliveries: &impl Deliveries) {
    // given: a claimed delivery
    deliveries
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    // when: it is refused until a later moment
    deliveries
        .mark_as_refused(&claimed[0], "the projection had not landed", at(1_010))
        .await
        .expect("nacking should succeed");

    // then: it is not due before then
    assert!(
        deliveries
            .claim_due(at(1_005), 16)
            .await
            .expect("claiming should succeed")
            .is_empty(),
        "a listener that failed because something had not landed yet needs time, not a \
         second try in the same breath"
    );

    // and: is claimed again then, its attempt counted
    let again = deliveries
        .claim_due(at(1_010), 16)
        .await
        .expect("claiming should succeed");

    assert_eq!(again.len(), 1);
    assert_eq!(
        again[0].attempts, 1,
        "the attempt count travels with the delivery, so a restart does not reset the budget"
    );
}

pub async fn a_delivery_given_up_on_becomes_a_dead_letter(deliveries: &impl Deliveries) {
    // given: a claimed delivery
    deliveries
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    // when: it is given up on
    deliveries
        .give_up(&claimed[0], "it refused five times", at(1_001))
        .await
        .expect("dead-lettering should succeed");

    // then: nothing waits
    assert_eq!(deliveries.waiting().await.expect("counting"), 0);
    // and: it is a dead letter saying for whom, why and when, not yet acknowledged
    let dead = deliveries
        .dead_letters()
        .await
        .expect("reading the dead letters should succeed");
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0].listener, a_listener("catalogue-idea"));
    assert_eq!(dead[0].why, "it refused five times");
    assert_eq!(
        dead[0].given_up_at,
        at(1_001),
        "an author told something was refused needs to know when, to tell it from what they did since"
    );
    assert_eq!(dead[0].acknowledged_at, None);
}

async fn a_dead_letter(deliveries: &impl Deliveries, routing: &str) -> DeadLetter {
    deliveries
        .enqueue(&a_listener("catalogue-idea"), &a_message(routing))
        .await
        .expect("enqueuing should succeed");
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");
    deliveries
        .give_up(&claimed[0], "it refused five times", at(1_001))
        .await
        .expect("dead-lettering should succeed");

    deliveries
        .dead_letters()
        .await
        .expect("reading the dead letters should succeed")
        .into_iter()
        .find(|dead| dead.message.routing.to_string() == routing)
        .expect("the message given up on is a dead letter")
}

pub async fn a_retried_dead_letter_is_delivered_afresh(deliveries: &impl Deliveries) {
    // given: a dead letter
    let dead = a_dead_letter(deliveries, "idea.captured").await;

    // when: it is retried for a later moment
    deliveries
        .retry(dead.id, at(2_000))
        .await
        .expect("retrying should succeed");

    // then: it is no dead letter any more
    assert!(
        deliveries
            .dead_letters()
            .await
            .expect("reading the dead letters should succeed")
            .is_empty(),
        "a retried message is offered again, not kept beside its new delivery"
    );
    // and: it is not due before that moment
    assert!(
        deliveries
            .claim_due(at(1_999), 16)
            .await
            .expect("claiming should succeed")
            .is_empty()
    );
    // and: then it is delivered afresh to its listener
    let again = deliveries
        .claim_due(at(2_000), 16)
        .await
        .expect("claiming should succeed");
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].listener, dead.listener);
    assert_eq!(again[0].message, dead.message);
    assert_eq!(
        again[0].attempts, 0,
        "a retry gets the whole budget, or a message refused until it died would die again on its \
         first refusal"
    );
}

pub async fn a_retried_dead_letter_wakes_whoever_is_waiting(deliveries: &impl Deliveries) {
    // given: a dead letter
    let dead = a_dead_letter(deliveries, "idea.captured").await;
    // and: someone waiting to be woken
    let mut notifications = deliveries
        .notifications()
        .await
        .expect("asking for notifications is not a failure");
    let _enqueued =
        tokio::time::timeout(std::time::Duration::from_millis(100), notifications.wait()).await;

    // when: it is retried
    deliveries
        .retry(dead.id, at(2_000))
        .await
        .expect("retrying should succeed");
    // then: whoever waits is woken
    let woken = tokio::time::timeout(std::time::Duration::from_secs(1), notifications.wait()).await;

    assert!(
        woken.is_ok(),
        "the author pressed Try again and is watching, so it has to go now rather than at the next poll"
    );
}

pub async fn an_acknowledged_dead_letter_is_kept_and_marked(deliveries: &impl Deliveries) {
    // given: two dead letters
    let acknowledged = a_dead_letter(deliveries, "idea.captured").await;
    let untouched = a_dead_letter(deliveries, "idea.retitled").await;

    // when: one is acknowledged
    deliveries
        .acknowledge(acknowledged.id, at(3_000))
        .await
        .expect("acknowledging should succeed");

    // then: both are kept, that one marked acknowledged
    let dead = deliveries
        .dead_letters()
        .await
        .expect("reading the dead letters should succeed");
    assert_eq!(
        dead,
        vec![
            DeadLetter {
                acknowledged_at: Some(at(3_000)),
                ..acknowledged
            },
            untouched,
        ],
        "acknowledging only quiets the alarm; the message stays, to be retried or repaired later, \
         since nothing can rebuild what a listener never saw"
    );
    // and: nothing waits
    assert_eq!(deliveries.waiting().await.expect("counting"), 0);
}

pub async fn acknowledging_again_keeps_the_first_moment(deliveries: &impl Deliveries) {
    // given: a dead letter
    let dead = a_dead_letter(deliveries, "idea.captured").await;

    // when: it is acknowledged twice
    deliveries
        .acknowledge(dead.id, at(3_000))
        .await
        .expect("acknowledging should succeed");
    deliveries
        .acknowledge(dead.id, at(4_000))
        .await
        .expect("acknowledging again should succeed");

    // then: it keeps the first moment
    let acknowledged = deliveries
        .dead_letters()
        .await
        .expect("reading the dead letters should succeed");
    assert_eq!(acknowledged[0].acknowledged_at, Some(at(3_000)));
}

pub async fn an_acknowledged_dead_letter_can_still_be_retried(deliveries: &impl Deliveries) {
    // given: an acknowledged dead letter
    let dead = a_dead_letter(deliveries, "idea.captured").await;
    deliveries
        .acknowledge(dead.id, at(3_000))
        .await
        .expect("acknowledging should succeed");

    // when: it is retried
    deliveries
        .retry(dead.id, at(4_000))
        .await
        .expect("retrying should succeed");

    // then: it is no dead letter any more
    assert!(
        deliveries
            .dead_letters()
            .await
            .expect("reading the dead letters should succeed")
            .is_empty()
    );
    // and: it is delivered again
    assert_eq!(
        deliveries
            .claim_due(at(4_000), 16)
            .await
            .expect("claiming should succeed")
            .len(),
        1
    );
}

pub async fn retrying_one_dead_letter_leaves_the_others(deliveries: &impl Deliveries) {
    // given: two dead letters
    let kept = a_dead_letter(deliveries, "idea.captured").await;
    let retried = a_dead_letter(deliveries, "idea.retitled").await;

    // when: one is retried
    deliveries
        .retry(retried.id, at(2_000))
        .await
        .expect("retrying should succeed");

    // then: the other stays a dead letter
    assert_eq!(
        deliveries
            .dead_letters()
            .await
            .expect("reading the dead letters should succeed"),
        vec![kept]
    );
    // and: one delivery waits
    assert_eq!(deliveries.waiting().await.expect("counting"), 1);
}

fn a_delivery_never_claimed() -> Queued {
    Queued {
        id: 404,
        claim: Claim::drawn(),
        listener: a_listener("catalogue-idea"),
        message: a_message("idea.captured"),
        attempts: 0,
    }
}

pub async fn handling_something_already_gone_is_harmless(deliveries: &impl Deliveries) {
    // expect: settling a delivery or dead letter that is not there succeeds
    deliveries
        .mark_as_handled(&a_delivery_never_claimed())
        .await
        .expect("acking an unknown delivery should not fail");
    deliveries
        .mark_as_refused(&a_delivery_never_claimed(), "nothing", at(1_000))
        .await
        .expect("nacking an unknown delivery should not fail");
    deliveries
        .give_up(&a_delivery_never_claimed(), "nothing", at(1_000))
        .await
        .expect("dead-lettering an unknown delivery should not fail");
    deliveries
        .retry(404, at(1_000))
        .await
        .expect("retrying twice, as a double click does, should not fail");
    deliveries
        .acknowledge(404, at(1_000))
        .await
        .expect("acknowledging an unknown dead letter should not fail");
}

pub async fn the_backoff_grows_with_each_attempt(deliveries: &impl Deliveries) {
    // expect: each attempt waits longer, from under a second to over a minute
    let _ = deliveries.waiting().await;

    assert!(
        delivering::again_after(1) < Duration::seconds(1),
        "the failure we actually have is a projection a beat behind, which clears at once"
    );
    assert!(
        delivering::again_after(1) < delivering::again_after(2)
            && delivering::again_after(2) < delivering::again_after(3),
        "a listener failing for a reason that will not clear soon should be asked less often"
    );
    assert!(
        delivering::again_after(ATTEMPTS) > Duration::minutes(1),
        "the tail has to outlast a database blinking, or an outage dead-letters everything that \
         was in flight"
    );
}

pub async fn deliveries_are_claimed_in_the_order_they_were_enqueued(deliveries: &impl Deliveries) {
    // given: six messages enqueued for a listener in turn
    let mut enqueued = Vec::new();
    for nth in 0..6 {
        let message = a_message(&format!("outline.step_{nth}"));
        deliveries
            .enqueue(&a_listener("index-outline-appearances"), &message)
            .await
            .expect("enqueuing should succeed");
        enqueued.push(message.id);
    }

    // when: what is due is claimed
    let claimed: Vec<_> = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed")
        .into_iter()
        .map(|queued| queued.message.id)
        .collect();

    // then: they come in the order they were enqueued
    assert_eq!(
        claimed, enqueued,
        "a listener handed a detach before the attach it undoes would end up wrong for good"
    );
}

pub async fn a_holder_whose_claim_ran_out_settles_nothing(deliveries: &impl Deliveries) {
    // given: a delivery whose first claim ran out
    deliveries
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");
    let stale = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");
    // and: a second claim holding it
    let holding = deliveries
        .claim_due(at(1_001) + CLAIM_FOR, 16)
        .await
        .expect("claiming again once the first claim ran out should succeed");

    // when: the first holder refuses it, gives up on it and handles it
    deliveries
        .mark_as_refused(&stale[0], "the first holder gave up", at(1_000))
        .await
        .expect("a stale refusal is no failure");
    deliveries
        .give_up(&stale[0], "the first holder gave up", at(1_000))
        .await
        .expect("a stale giving up is no failure");
    deliveries
        .mark_as_handled(&stale[0])
        .await
        .expect("a stale ack is no failure");

    // then: it still waits, alive and held
    assert_eq!(
        (
            deliveries.waiting().await.expect("counting"),
            deliveries
                .dead_letters()
                .await
                .expect("reading the dead letters should succeed")
                .len(),
            deliveries
                .claim_due(at(1_002) + CLAIM_FOR, 16)
                .await
                .expect("claiming should succeed")
                .len(),
        ),
        (1, 0, 0),
        "whoever lost the claim neither acks, refuses nor buries what another now handles"
    );
    // when: the second holder handles it
    deliveries
        .mark_as_handled(&holding[0])
        .await
        .expect("acking should succeed");
    // then: nothing waits
    assert_eq!(
        deliveries.waiting().await.expect("counting"),
        0,
        "the one holding the claim settles it"
    );
}

pub async fn a_message_queued_twice_for_a_listener_is_one_delivery(deliveries: &impl Deliveries) {
    // when: a message is enqueued twice for one listener
    let message = a_message("idea.captured");
    for _ in 0..2 {
        deliveries
            .enqueue(&a_listener("catalogue-idea"), &message)
            .await
            .expect("enqueuing a duplicate is no failure");
    }

    // then: one delivery is due
    assert_eq!(
        deliveries
            .claim_due(at(1_000), 16)
            .await
            .expect("claiming should succeed")
            .len(),
        1,
        "a message relayed twice is handled once while it waits"
    );
}

pub async fn a_dead_letter_retried_while_its_message_waits_again_stays_one_delivery(
    deliveries: &impl Deliveries,
) {
    // given: a dead letter
    let dead = a_dead_letter(deliveries, "idea.captured").await;
    // and: its message enqueued again
    deliveries
        .enqueue(&dead.listener, &dead.message)
        .await
        .expect("enqueuing should succeed");

    // when: it is retried
    deliveries
        .retry(dead.id, at(2_000))
        .await
        .expect("retrying should succeed");

    // then: one delivery waits
    assert_eq!(deliveries.waiting().await.expect("counting"), 1);
    // and: no dead letter is left
    assert!(
        deliveries
            .dead_letters()
            .await
            .expect("reading the dead letters should succeed")
            .is_empty()
    );
}

#[macro_export]
macro_rules! delivery_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::deliveries::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::deliveries::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::delivery_conformance_case!(
            $workbench,
            an_enqueued_delivery_is_claimed_when_it_is_due
        );
        $crate::delivery_conformance_case!(
            $workbench,
            one_message_becomes_one_delivery_per_listener
        );
        $crate::delivery_conformance_case!($workbench, a_handled_delivery_is_gone);
        $crate::delivery_conformance_case!($workbench, a_claimed_delivery_is_not_handed_out_twice);
        $crate::delivery_conformance_case!(
            $workbench,
            a_refused_delivery_waits_and_counts_the_attempt
        );
        $crate::delivery_conformance_case!(
            $workbench,
            a_delivery_given_up_on_becomes_a_dead_letter
        );
        $crate::delivery_conformance_case!($workbench, a_retried_dead_letter_is_delivered_afresh);
        $crate::delivery_conformance_case!(
            $workbench,
            a_retried_dead_letter_wakes_whoever_is_waiting
        );
        $crate::delivery_conformance_case!(
            $workbench,
            an_acknowledged_dead_letter_is_kept_and_marked
        );
        $crate::delivery_conformance_case!($workbench, acknowledging_again_keeps_the_first_moment);
        $crate::delivery_conformance_case!(
            $workbench,
            an_acknowledged_dead_letter_can_still_be_retried
        );
        $crate::delivery_conformance_case!($workbench, retrying_one_dead_letter_leaves_the_others);
        $crate::delivery_conformance_case!($workbench, handling_something_already_gone_is_harmless);
        $crate::delivery_conformance_case!($workbench, the_backoff_grows_with_each_attempt);
        $crate::delivery_conformance_case!(
            $workbench,
            deliveries_are_claimed_in_the_order_they_were_enqueued
        );
        $crate::delivery_conformance_case!(
            $workbench,
            a_holder_whose_claim_ran_out_settles_nothing
        );
        $crate::delivery_conformance_case!(
            $workbench,
            a_message_queued_twice_for_a_listener_is_one_delivery
        );
        $crate::delivery_conformance_case!(
            $workbench,
            a_dead_letter_retried_while_its_message_waits_again_stays_one_delivery
        );
    };
}
