use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use clock::Clock;
use messaging::{Deliveries, RETRIABLE_FOR};

use crate::Chore;

const DEAD_LETTERS_EACH_BATCH: i64 = 500;

pub struct ForgetOldDeadLetters {
    deliveries: Arc<dyn Deliveries>,
    clock: Arc<dyn Clock>,
}

impl ForgetOldDeadLetters {
    pub fn new(deliveries: Arc<dyn Deliveries>, clock: Arc<dyn Clock>) -> Self {
        Self { deliveries, clock }
    }
}

#[async_trait]
impl Chore for ForgetOldDeadLetters {
    fn named(&self) -> &'static str {
        "forget-old-dead-letters"
    }

    fn every(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let before = self.clock.now() - RETRIABLE_FOR;
        let mut forgotten = 0;

        loop {
            let batch = self
                .deliveries
                .forget_old_dead_letters(before, DEAD_LETTERS_EACH_BATCH)
                .await?;
            forgotten += batch;
            if batch < DEAD_LETTERS_EACH_BATCH as u64 {
                return Ok(forgotten);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clock::FixedClock;
    use messaging::{InMemoryDeliveries, ListenerName, Message, RoutingKey};
    use time::OffsetDateTime;
    use time::macros::datetime;

    use super::*;

    const NOW: OffsetDateTime = datetime!(2026-11-10 12:00 UTC);

    async fn dead_letter_of(
        deliveries: &InMemoryDeliveries,
        routing: &str,
        occurred: OffsetDateTime,
    ) {
        deliveries
            .enqueue(
                &ListenerName::parse("catalogue-idea").expect("a plain name"),
                &Message::opening(
                    RoutingKey::parse(routing).expect("a plain key"),
                    serde_json::json!({ "email": "ada@example.com" }),
                    occurred,
                ),
            )
            .await
            .expect("enqueuing should succeed");
        let claimed = deliveries
            .claim_due(NOW, 16)
            .await
            .expect("claiming should succeed");
        deliveries
            .give_up(&claimed[0], "it refused five times", NOW)
            .await
            .expect("dead-lettering should succeed");
    }

    #[tokio::test]
    async fn dead_letters_too_old_to_try_again_are_forgotten_every_hour() {
        // given: a dead letter a moment too old to be tried again, one as old and acknowledged, and one just young enough
        let deliveries = Arc::new(InMemoryDeliveries::new());
        dead_letter_of(
            &deliveries,
            "account.email.verified",
            NOW - RETRIABLE_FOR - time::Duration::SECOND,
        )
        .await;
        let heard = deliveries
            .dead_letters()
            .await
            .expect("reading should succeed")[0]
            .id;
        deliveries
            .acknowledge(heard, NOW)
            .await
            .expect("acknowledging should succeed");
        dead_letter_of(
            &deliveries,
            "account.signed.up",
            NOW - RETRIABLE_FOR - time::Duration::SECOND,
        )
        .await;
        dead_letter_of(&deliveries, "idea.captured", NOW - RETRIABLE_FOR).await;
        let chore = ForgetOldDeadLetters::new(deliveries.clone(), Arc::new(FixedClock::new(NOW)));

        // when: the chore is carried out
        let forgotten = chore.carry_out().await.expect("forgetting should succeed");

        // then: the acknowledged one is forgotten whole, and what the other old one told
        assert_eq!(forgotten, 2);
        let told: Vec<(String, bool)> = deliveries
            .dead_letters()
            .await
            .expect("reading should succeed")
            .into_iter()
            .map(|dead| {
                (
                    dead.message.routing.to_string(),
                    dead.message.payload.is_null(),
                )
            })
            .collect();
        assert_eq!(
            told,
            [
                ("account.signed.up".to_owned(), true),
                ("idea.captured".to_owned(), false)
            ]
        );
        // and: it comes round every hour
        assert_eq!(chore.every().as_secs(), 3_600);
    }
}
