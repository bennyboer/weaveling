use async_trait::async_trait;
use messaging::{Failure, Handler, Message};
use notifications_contract::{DISMISSED, NOTED};
use notifications_core::{NoticeId, NotificationsCore};

pub struct CatalogueNoticeListener {
    notifications: NotificationsCore,
}

impl CatalogueNoticeListener {
    pub fn new(notifications: NotificationsCore) -> Self {
        Self { notifications }
    }
}

#[async_trait]
impl Handler for CatalogueNoticeListener {
    const NAME: &'static str = "catalogue-notice";
    const LISTENS_TO: &'static [&'static str] = &[NOTED, DISMISSED];
    const WHEN_REFUSED: &'static str = "A notice did not reach its account's list of notices.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .notifications
            .catalogue(&eventpublishing::aggregate_in::<NoticeId>(message)?)
            .await?)
    }
}
