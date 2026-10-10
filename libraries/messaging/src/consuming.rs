use std::error::Error;
use std::sync::Arc;

use clock::Clock;

use tokio::sync::watch;

use crate::delivering::{ATTEMPTS, Deliveries, again_after};
use crate::in_process::InProcessDispatcher;
use crate::listening::{Delivery, NotHandled, Notifications};

pub const LOOK_EVERY: std::time::Duration = std::time::Duration::from_millis(200);

pub const AT_MOST: i64 = 128;

fn down_to_the_cause(refused: &NotHandled) -> String {
    let mut saying = refused.to_string();
    let mut cause = Error::source(refused);

    while let Some(next) = cause {
        saying.push_str(": ");
        saying.push_str(&next.to_string());
        cause = next.source();
    }

    saying
}

pub struct DeliveryConsumer {
    dispatcher: Arc<InProcessDispatcher>,
    deliveries: Arc<dyn Deliveries>,
    clock: Arc<dyn Clock>,
    attempts: i32,
    look_every: std::time::Duration,
    at_most: i64,
}

impl DeliveryConsumer {
    pub fn new(
        dispatcher: Arc<InProcessDispatcher>,
        deliveries: Arc<dyn Deliveries>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            dispatcher,
            deliveries,
            clock,
            attempts: ATTEMPTS,
            look_every: LOOK_EVERY,
            at_most: AT_MOST,
        }
    }

    pub fn giving_up_after(mut self, attempts: i32) -> Self {
        self.attempts = attempts;
        self
    }

    pub fn looking_every(mut self, look_every: std::time::Duration) -> Self {
        self.look_every = look_every;
        self
    }

    pub async fn run(self, mut stopped: watch::Receiver<bool>) {
        let mut notifications = match self.deliveries.notifications().await {
            Ok(notifications) => Some(notifications),
            Err(why) => {
                tracing::warn!(error = %why, "the deliveries cannot be listened to, so the consumer polls alone");

                None
            }
        };

        loop {
            self.drain(self.at_most).await;

            if !self.waited(notifications.as_mut(), &mut stopped).await {
                break;
            }
        }
    }

    async fn waited(
        &self,
        notifications: Option<&mut Box<dyn Notifications>>,
        stopped: &mut watch::Receiver<bool>,
    ) -> bool {
        let Some(notifications) = notifications else {
            return tokio::select! {
                _ = stopped.changed() => false,
                _ = tokio::time::sleep(self.look_every) => true,
            };
        };

        tokio::select! {
            _ = stopped.changed() => false,
            _ = tokio::time::sleep(self.look_every) => true,
            () = notifications.wait() => true,
        }
    }

    pub async fn drain(&self, at_most: i64) -> usize {
        let now = self.clock.now();

        let claimed = match self.deliveries.claim_due(now, at_most).await {
            Ok(claimed) => claimed,
            Err(why) => {
                tracing::error!(error = %why, "the deliveries could not be claimed");

                return 0;
            }
        };
        let taken = claimed.len();

        for queued in claimed {
            let Some(listener) = self.dispatcher.named(&queued.listener) else {
                tracing::warn!(
                    listener = %queued.listener,
                    "a delivery names a listener nobody registered, so it waits for one that does"
                );

                continue;
            };

            let refused = match listener.handle(&queued.message).await {
                Ok(()) => {
                    self.settled(self.deliveries.mark_as_handled(queued.id).await);

                    continue;
                }
                Err(refused) => refused,
            };

            if listener.delivery() == Delivery::Fleeting {
                tracing::debug!(
                    listener = %queued.listener,
                    error = %refused,
                    "a fleeting listener let a message go by"
                );
                self.settled(self.deliveries.mark_as_handled(queued.id).await);

                continue;
            }

            let spent = queued.attempts + 1 >= self.attempts;
            let why = down_to_the_cause(&refused);

            if spent {
                tracing::error!(
                    listener = %queued.listener,
                    routing = %queued.message.routing,
                    message = %queued.message.id,
                    attempts = queued.attempts + 1,
                    error = %why,
                    "a listener refused a message until the attempts ran out"
                );
                self.settled(self.deliveries.give_up(queued.id, &why, now).await);
            } else {
                tracing::warn!(
                    listener = %queued.listener,
                    routing = %queued.message.routing,
                    attempts = queued.attempts + 1,
                    error = %why,
                    "a listener refused a message, so it will be offered again"
                );
                let again_at = now + again_after(queued.attempts + 1);
                self.settled(
                    self.deliveries
                        .mark_as_refused(queued.id, &why, again_at)
                        .await,
                );
            }
        }

        taken
    }

    fn settled<E: std::fmt::Display>(&self, written: Result<(), E>) {
        if let Err(why) = written {
            tracing::error!(error = %why, "a delivery could not be settled, so it will be offered again when its claim expires");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use clock::FixedClock;
    use serde_json::json;
    use thiserror::Error;
    use time::{Duration, OffsetDateTime};

    use super::*;
    use crate::deliveries::InMemoryDeliveries;
    use crate::delivering::DeadLetter;
    use crate::listening::{Listener, ListenerName, NotHandled, Publisher};
    use crate::message::Message;
    use crate::routing::{RoutingKey, Subscription};

    #[derive(Debug, Error)]
    #[error("the projection had not landed")]
    struct Lagging;

    struct Fussy {
        name: &'static str,
        delivery: Delivery,
        refusals: AtomicUsize,
        heard: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl Listener for Fussy {
        fn named(&self) -> ListenerName {
            ListenerName::parse(self.name).expect("a plain name is fine")
        }

        fn listens_to(&self) -> Vec<Subscription> {
            vec![Subscription::parse("#").expect("a plain pattern is fine")]
        }

        fn delivery(&self) -> Delivery {
            self.delivery
        }

        async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
            if self.refusals.load(Ordering::SeqCst) > 0 {
                self.refusals.fetch_sub(1, Ordering::SeqCst);

                return Err(NotHandled::because(
                    self.named(),
                    message.routing.clone(),
                    Lagging,
                ));
            }

            self.heard
                .lock()
                .expect("lock poisoned")
                .push(message.routing.to_string());

            Ok(())
        }
    }

    impl Fussy {
        fn refusing(name: &'static str, times: usize) -> Arc<Self> {
            Arc::new(Self {
                name,
                delivery: Delivery::Kept,
                refusals: AtomicUsize::new(times),
                heard: Mutex::new(Vec::new()),
            })
        }

        fn fleeting(name: &'static str) -> Arc<Self> {
            Arc::new(Self {
                name,
                delivery: Delivery::Fleeting,
                refusals: AtomicUsize::new(usize::MAX),
                heard: Mutex::new(Vec::new()),
            })
        }

        fn how_many(&self) -> usize {
            self.heard.lock().expect("lock poisoned").len()
        }
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn saying(routing: &str) -> Message {
        Message::opening(
            RoutingKey::parse(routing).expect("a plain key is fine"),
            json!({ "nothing": "much" }),
            at(1_000),
        )
    }

    struct Wired {
        dispatcher: Arc<InProcessDispatcher>,
        deliveries: Arc<InMemoryDeliveries>,
        clock: Arc<FixedClock>,
    }

    fn a_workbench() -> Wired {
        let deliveries = Arc::new(InMemoryDeliveries::new());
        let clock = Arc::new(FixedClock::new(at(1_000)));

        Wired {
            dispatcher: Arc::new(InProcessDispatcher::queueing_to(deliveries.clone())),
            deliveries,
            clock,
        }
    }

    impl Wired {
        fn consumer(&self) -> DeliveryConsumer {
            DeliveryConsumer::new(
                self.dispatcher.clone(),
                self.deliveries.clone(),
                self.clock.clone(),
            )
        }

        async fn publish(&self, routing: &str) {
            self.dispatcher
                .publish(saying(routing))
                .await
                .expect("handing a message over should succeed");
        }

        async fn waiting(&self) -> usize {
            self.deliveries.waiting().await.expect("counting")
        }

        async fn dead(&self) -> Vec<DeadLetter> {
            self.deliveries.dead_letters().await.expect("reading")
        }
    }

    #[tokio::test]
    async fn a_listener_that_refuses_once_is_offered_the_message_again() {
        let wired = a_workbench();
        let fussy = Fussy::refusing("catalogue-idea", 1);
        wired.dispatcher.listen(fussy.clone());
        wired.publish("idea.captured").await;

        wired.consumer().drain(16).await;
        assert_eq!(fussy.how_many(), 0, "the first offer was refused");
        assert_eq!(wired.waiting().await, 1);

        wired
            .clock
            .set(at(1_000) + again_after(1) + Duration::seconds(1));
        wired.consumer().drain(16).await;

        assert_eq!(
            fussy.how_many(),
            1,
            "a refusal is a retry, which is the whole of what Delivery::Kept promises"
        );
        assert_eq!(wired.waiting().await, 0);
    }

    #[tokio::test]
    async fn a_retry_reaches_only_the_listener_that_refused() {
        let wired = a_workbench();
        let coping = Fussy::refusing("catalogue-idea", 0);
        let fussy = Fussy::refusing("unpin-discarded-idea", 1);
        wired.dispatcher.listen(coping.clone());
        wired.dispatcher.listen(fussy.clone());
        wired.publish("idea.discarded").await;

        wired.consumer().drain(16).await;
        wired
            .clock
            .set(at(1_000) + again_after(1) + Duration::seconds(1));
        wired.consumer().drain(16).await;

        assert_eq!(fussy.how_many(), 1);
        assert_eq!(
            coping.how_many(),
            1,
            "each listener has its own queue and acks on its own, so one failing must never \
             hand the message to another a second time"
        );
    }

    #[tokio::test]
    async fn a_listener_that_never_copes_ends_up_in_the_dead_letters() {
        let wired = a_workbench();
        wired
            .dispatcher
            .listen(Fussy::refusing("catalogue-idea", 100));
        wired.publish("idea.captured").await;

        let mut now = at(1_000);
        for attempt in 1..=ATTEMPTS {
            wired.clock.set(now);
            wired.consumer().drain(16).await;
            now = now + again_after(attempt) + Duration::seconds(1);
        }

        assert_eq!(
            wired.waiting().await,
            0,
            "a message nobody can handle must not sit in the queue for ever"
        );
        let dead = wired.dead().await;
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].attempts, ATTEMPTS);
        assert_eq!(dead[0].listener.as_str(), "catalogue-idea");
        assert!(
            dead[0].why.contains("had not landed"),
            "a dead letter has to say why, or nobody can act on it: {}",
            dead[0].why
        );
    }

    #[tokio::test]
    async fn a_fleeting_listener_refusing_is_dropped_rather_than_retried() {
        let wired = a_workbench();
        wired.dispatcher.listen(Fussy::fleeting("board-live"));
        wired.publish("board.pinned").await;

        wired.consumer().drain(16).await;

        assert_eq!(wired.waiting().await, 0);
        assert!(
            wired.dead().await.is_empty(),
            "a fleeting message is superseded by the next one, so keeping it would be noise"
        );
    }

    #[tokio::test]
    async fn a_delivery_for_a_listener_nobody_registered_waits_rather_than_dying() {
        let wired = a_workbench();
        wired
            .deliveries
            .enqueue(
                &ListenerName::parse("not-wired-yet").expect("a plain name is fine"),
                &saying("idea.captured"),
            )
            .await
            .expect("enqueuing should succeed");

        wired.consumer().drain(16).await;

        assert_eq!(
            wired.waiting().await,
            1,
            "a listener missing at startup is a wiring fault to fix, not a message to throw away"
        );
        assert!(wired.dead().await.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_delivery_is_taken_long_before_the_next_look_would() {
        let wired = a_workbench();
        let coping = Fussy::refusing("catalogue-idea", 0);
        wired.dispatcher.listen(coping.clone());

        let (stopping, stopped) = watch::channel(false);
        let running = tokio::spawn(
            DeliveryConsumer::new(
                wired.dispatcher.clone(),
                wired.deliveries.clone(),
                wired.clock.clone(),
            )
            .looking_every(std::time::Duration::from_secs(300))
            .run(stopped),
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        wired.publish("idea.captured").await;

        let mut took = false;
        for _ in 0..40 {
            if coping.how_many() == 1 {
                took = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        assert!(
            took,
            "looking is five minutes away, so only the notification can have woken the consumer"
        );

        let _ = stopping.send(true);
        let _ = running.await;
    }
}
