use std::error::Error;
use std::time::Duration;

use async_trait::async_trait;
use chores::Chore;
use scenes_sync::LiveScenes;

const EVERY: Duration = Duration::from_secs(60);
const AT_MOST: usize = 100;

pub struct CountTheUncounted {
    live: LiveScenes,
}

impl CountTheUncounted {
    pub fn of(live: LiveScenes) -> Self {
        Self { live }
    }
}

#[async_trait]
impl Chore for CountTheUncounted {
    fn named(&self) -> &'static str {
        "count-scenes-left-uncounted"
    }

    fn every(&self) -> Duration {
        EVERY
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self.live.count_the_uncounted(AT_MOST).await?)
    }
}
