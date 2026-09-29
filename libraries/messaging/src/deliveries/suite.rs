use time::{Duration, OffsetDateTime};

use crate::delivering::{ATTEMPTS, Deliveries, again_after};
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
    deliveries
        .enqueue(&a_listener("catalogue-piece"), &a_message("piece.captured"))
        .await
        .expect("enqueuing should succeed");

    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].listener, a_listener("catalogue-piece"));
    assert_eq!(claimed[0].attempts, 0);
}

pub async fn one_message_becomes_one_delivery_per_listener(deliveries: &impl Deliveries) {
    let message = a_message("piece.discarded");
    for listener in ["unpin-discarded-piece", "detach-discarded-piece"] {
        deliveries
            .enqueue(&a_listener(listener), &message)
            .await
            .expect("enqueuing should succeed");
    }

    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    assert_eq!(
        claimed.len(),
        2,
        "each listener has its own queue, so one failing must not redeliver to the other"
    );
}

pub async fn a_handled_delivery_is_gone(deliveries: &impl Deliveries) {
    deliveries
        .enqueue(&a_listener("catalogue-piece"), &a_message("piece.captured"))
        .await
        .expect("enqueuing should succeed");
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    deliveries
        .mark_as_handled(claimed[0].id)
        .await
        .expect("acking should succeed");

    assert_eq!(deliveries.waiting().await.expect("counting"), 0);
}

pub async fn a_claimed_delivery_is_not_handed_out_twice(deliveries: &impl Deliveries) {
    deliveries
        .enqueue(&a_listener("catalogue-piece"), &a_message("piece.captured"))
        .await
        .expect("enqueuing should succeed");
    deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    let again = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    assert!(
        again.is_empty(),
        "two workers draining at once must not hand one delivery to a listener twice"
    );
}

pub async fn a_refused_delivery_waits_and_counts_the_attempt(deliveries: &impl Deliveries) {
    deliveries
        .enqueue(&a_listener("catalogue-piece"), &a_message("piece.captured"))
        .await
        .expect("enqueuing should succeed");
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    deliveries
        .mark_as_refused(claimed[0].id, "the projection had not landed", at(1_010))
        .await
        .expect("nacking should succeed");

    assert!(
        deliveries
            .claim_due(at(1_005), 16)
            .await
            .expect("claiming should succeed")
            .is_empty(),
        "a listener that failed because something had not landed yet needs time, not a \
         second try in the same breath"
    );

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
    deliveries
        .enqueue(&a_listener("catalogue-piece"), &a_message("piece.captured"))
        .await
        .expect("enqueuing should succeed");
    let claimed = deliveries
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    deliveries
        .give_up(claimed[0].id, "it refused five times")
        .await
        .expect("dead-lettering should succeed");

    assert_eq!(deliveries.waiting().await.expect("counting"), 0);
    let dead = deliveries
        .dead_letters()
        .await
        .expect("reading the dead letters should succeed");
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0].listener, a_listener("catalogue-piece"));
    assert_eq!(dead[0].why, "it refused five times");
}

pub async fn handling_something_already_gone_is_harmless(deliveries: &impl Deliveries) {
    deliveries
        .mark_as_handled(404)
        .await
        .expect("acking an unknown delivery should not fail");
    deliveries
        .mark_as_refused(404, "nothing", at(1_000))
        .await
        .expect("nacking an unknown delivery should not fail");
    deliveries
        .give_up(404, "nothing")
        .await
        .expect("dead-lettering an unknown delivery should not fail");
}

pub async fn the_backoff_grows_with_each_attempt(deliveries: &impl Deliveries) {
    let _ = deliveries.waiting().await;

    assert!(
        again_after(1) < Duration::seconds(1),
        "the failure we actually have is a projection a beat behind, which clears at once"
    );
    assert!(
        again_after(1) < again_after(2) && again_after(2) < again_after(3),
        "a listener failing for a reason that will not clear soon should be asked less often"
    );
    assert!(
        again_after(ATTEMPTS) > Duration::minutes(1),
        "the tail has to outlast a database blinking, or an outage dead-letters everything          that was in flight"
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
        $crate::delivery_conformance_case!($workbench, handling_something_already_gone_is_harmless);
        $crate::delivery_conformance_case!($workbench, the_backoff_grows_with_each_attempt);
    };
}
