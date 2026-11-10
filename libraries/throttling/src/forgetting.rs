use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chores::Chore;
use clock::Clock;

use crate::{Throttles, Tries};

const TRIES_EACH_BATCH: u64 = 500;

pub struct ForgetOldTries {
    named: &'static str,
    throttles: Arc<dyn Throttles>,
    clock: Arc<dyn Clock>,
}

impl ForgetOldTries {
    pub fn new(named: &'static str, throttles: Arc<dyn Throttles>, clock: Arc<dyn Clock>) -> Self {
        Self {
            named,
            throttles,
            clock,
        }
    }
}

#[async_trait]
impl Chore for ForgetOldTries {
    fn named(&self) -> &'static str {
        self.named
    }

    fn every(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let before = self.clock.now() - Tries::FORGOTTEN_AFTER;
        let mut forgotten = 0;

        loop {
            let batch = self
                .throttles
                .forget_tried_before(before, TRIES_EACH_BATCH)
                .await?;
            forgotten += batch;
            if batch < TRIES_EACH_BATCH {
                return Ok(forgotten);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clock::FixedClock;
    use time::Duration;

    use super::*;
    use crate::{InMemoryThrottles, Rule};

    #[tokio::test]
    async fn counts_a_day_old_and_more_are_forgotten_every_hour_and_younger_ones_kept() {
        // given: a count tried a day and a second ago, and one tried a second short of a day ago
        let now = clock::at(1_000_000);
        let throttles = Arc::new(InMemoryThrottles::default());
        for (about, at) in [
            (
                "ada@example.com",
                now - Tries::FORGOTTEN_AFTER - Duration::SECOND,
            ),
            (
                "bea@example.com",
                now - Tries::FORGOTTEN_AFTER + Duration::SECOND,
            ),
        ] {
            throttles
                .admit("signing-in", about, Rule { free: 5 }, at)
                .await
                .expect("admitting should succeed");
        }
        let chore = ForgetOldTries::new(
            "forget-old-tries",
            throttles.clone(),
            Arc::new(FixedClock::new(now)),
        );

        // when: the chore is carried out
        let forgotten = chore.carry_out().await.expect("forgetting should succeed");

        // then: the old count alone is forgotten
        assert_eq!(forgotten, 1);
        assert!(
            throttles
                .tries("signing-in", "ada@example.com")
                .await
                .expect("reading should succeed")
                .is_none()
        );
        assert!(
            throttles
                .tries("signing-in", "bea@example.com")
                .await
                .expect("reading should succeed")
                .is_some()
        );
        // and: it comes round every hour
        assert_eq!(chore.every().as_secs(), 3_600);
    }
}
