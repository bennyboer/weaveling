use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use clock::Clock;
use messaging::RETRIABLE_FOR;
use time::OffsetDateTime;

use crate::Chore;

pub const TOMBSTONES_KEPT_FOR: time::Duration = RETRIABLE_FOR;

#[async_trait]
pub trait Tombstones: Send + Sync {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
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
        self.tombstones
            .forget_laid_before(self.clock.now() - TOMBSTONES_KEPT_FOR)
            .await
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
    }

    impl Graveyard {
        fn laid_at(at: &[OffsetDateTime]) -> Arc<Self> {
            Arc::new(Self {
                laid: Mutex::new(at.to_vec()),
            })
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
        ) -> Result<u64, Box<dyn Error + Send + Sync>> {
            let mut laid = self.laid.lock().expect("not poisoned");
            let had = laid.len();
            laid.retain(|at| *at >= before);

            Ok(u64::try_from(had - laid.len())?)
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
    async fn tombstones_are_forgotten_once_a_change_about_them_may_no_longer_be_tried_again() {
        let kept = NOW - RETRIABLE_FOR;
        let graveyard = Graveyard::laid_at(&[
            kept - time::Duration::seconds(1),
            kept,
            NOW - time::Duration::days(1),
        ]);

        let cleared = forgetting(&graveyard)
            .carry_out()
            .await
            .expect("forgetting should succeed");

        assert_eq!(cleared, 1);
        assert_eq!(
            graveyard.left(),
            vec![kept, NOW - time::Duration::days(1)],
            "a late change still tried again must find what it is about gone"
        );
    }

    #[test]
    fn the_chore_goes_by_the_name_its_feature_gives_it_and_comes_round_hourly() {
        let chore = forgetting(&Arc::default());

        assert_eq!(chore.named(), "forget-old-tombstones-of-somewhere");
        assert_eq!(chore.every(), Duration::from_secs(60 * 60));
    }
}
