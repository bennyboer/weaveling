use std::error::Error;
use std::time::Duration;

use async_trait::async_trait;
use chores::Chore;
use notifications_core::NotificationsCore;

const AT_MOST_EACH_ROUND: usize = 100;

pub struct ForgetOldNotices {
    notifications: NotificationsCore,
}

impl ForgetOldNotices {
    pub fn new(notifications: NotificationsCore) -> Self {
        Self { notifications }
    }
}

#[async_trait]
impl Chore for ForgetOldNotices {
    fn named(&self) -> &'static str {
        "forget-old-notices"
    }

    fn every(&self) -> Duration {
        Duration::from_secs(5 * 60)
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let forgotten = self.notifications.forget_due(AT_MOST_EACH_ROUND).await?;

        Ok(u64::try_from(forgotten)?)
    }
}
