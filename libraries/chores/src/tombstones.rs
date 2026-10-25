use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use clock::Clock;
use messaging::RETRIABLE_FOR;
use time::OffsetDateTime;

use crate::Chore;

pub const TOMBSTONES_KEPT_FOR: time::Duration = RETRIABLE_FOR.saturating_add(time::Duration::DAY);

pub const TOMBSTONES_EACH_BATCH: u64 = 500;

#[async_trait]
pub trait Tombstones: Send + Sync {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>>;
}

pub struct ForgetOldTombstones {
    named: &'static str,
    tombstones: Arc<dyn Tombstones>,
    clock: Arc<dyn Clock>,
}

impl ForgetOldTombstones {
    pub fn new(
        named: &'static str,
        tombstones: Arc<dyn Tombstones>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            named,
            tombstones,
            clock,
        }
    }
}

#[async_trait]
impl Chore for ForgetOldTombstones {
    fn named(&self) -> &'static str {
        self.named
    }

    fn every(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let before = self.clock.now() - TOMBSTONES_KEPT_FOR;
        let mut forgotten = 0;

        loop {
            let batch = self
                .tombstones
                .forget_laid_before(before, TOMBSTONES_EACH_BATCH)
                .await?;
            forgotten += batch;
            if batch < TOMBSTONES_EACH_BATCH {
                return Ok(forgotten);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use clock::FixedClock;
    use time::macros::datetime;

    use super::*;

    const NOW: OffsetDateTime = datetime!(2026-10-25 12:00 UTC);

    #[derive(Default)]
    struct Graveyard {
        laid: Mutex<Vec<OffsetDateTime>>,
        asked: Mutex<Vec<u64>>,
    }

    impl Graveyard {
        fn laid_at(at: &[OffsetDateTime]) -> Arc<Self> {
            Arc::new(Self {
                laid: Mutex::new(at.to_vec()),
                asked: Mutex::default(),
            })
        }

        fn batches_asked(&self) -> Vec<u64> {
            self.asked.lock().expect("not poisoned").clone()
        }

        fn left(&self) -> Vec<OffsetDateTime> {
            self.laid.lock().expect("not poisoned").clone()
        }
    }

    #[async_trait]
    impl Tombstones for Graveyard {
        async fn forget_laid_before(
            &self,
            before: OffsetDateTime,
            at_most: u64,
        ) -> Result<u64, Box<dyn Error + Send + Sync>> {
            self.asked.lock().expect("not poisoned").push(at_most);
            let mut laid = self.laid.lock().expect("not poisoned");
            let mut forgotten = 0;
            while forgotten < at_most {
                let Some(oldest) = laid.iter().position(|at| *at < before) else {
                    break;
                };
                laid.remove(oldest);
                forgotten += 1;
            }

            Ok(forgotten)
        }
    }

    fn forgetting(graveyard: &Arc<Graveyard>) -> ForgetOldTombstones {
        ForgetOldTombstones::new(
            "forget-old-tombstones-of-somewhere",
            graveyard.clone(),
            Arc::new(FixedClock::new(NOW)),
        )
    }

    #[tokio::test]
    async fn tombstones_are_forgotten_a_day_after_a_change_about_them_may_no_longer_be_retried() {
        let kept = NOW - RETRIABLE_FOR - time::Duration::DAY;
        let graveyard = Graveyard::laid_at(&[
            kept - time::Duration::seconds(1),
            kept,
            NOW - RETRIABLE_FOR - time::Duration::seconds(1),
        ]);

        let cleared = forgetting(&graveyard)
            .carry_out()
            .await
            .expect("forgetting should succeed");

        assert_eq!(cleared, 1);
        assert_eq!(
            graveyard.left(),
            vec![kept, NOW - RETRIABLE_FOR - time::Duration::seconds(1)],
            "a change about it may have happened a little after it was laid, and is retried as long \
             after that"
        );
    }

    #[tokio::test]
    async fn a_backlog_is_forgotten_batch_by_batch_until_one_comes_back_short() {
        let long_gone = NOW - TOMBSTONES_KEPT_FOR - time::Duration::DAY;
        let backlog = usize::try_from(TOMBSTONES_EACH_BATCH * 2 + 1).expect("a small number");
        let mut laid = vec![long_gone; backlog];
        laid.push(NOW);
        let graveyard = Graveyard::laid_at(&laid);

        let cleared = forgetting(&graveyard)
            .carry_out()
            .await
            .expect("forgetting should succeed");

        assert_eq!(cleared, TOMBSTONES_EACH_BATCH * 2 + 1);
        assert_eq!(graveyard.left(), vec![NOW]);
        assert_eq!(
            graveyard.batches_asked(),
            vec![TOMBSTONES_EACH_BATCH; 3],
            "each batch is its own short work, and a short one says nothing is left"
        );
    }

    #[test]
    fn the_chore_goes_by_the_name_its_feature_gives_it_and_comes_round_hourly() {
        let chore = forgetting(&Arc::default());

        assert_eq!(chore.named(), "forget-old-tombstones-of-somewhere");
        assert_eq!(chore.every(), Duration::from_secs(60 * 60));
    }
}
