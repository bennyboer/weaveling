use async_trait::async_trait;
use inbox::{KeptHandler, Refused};
use messaging::Message;
use notifications_contract::{DISMISSED, NOTED};
use notifications_core::{NoticeId, NotificationsCore};
use transacting::Transaction;

pub struct CatalogueNoticeListener {
    notifications: NotificationsCore,
}

impl CatalogueNoticeListener {
    pub fn new(notifications: NotificationsCore) -> Self {
        Self { notifications }
    }
}

#[async_trait]
impl KeptHandler for CatalogueNoticeListener {
    const NAME: &'static str = "catalogue-notice";
    const LISTENS_TO: &'static [&'static str] = &[NOTED, DISMISSED];
    const WHEN_REFUSED: &'static str = "A notice did not reach its account's list of notices.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        self.notifications
            .catalogue(
                Some(transaction),
                &eventpublishing::aggregate_in::<NoticeId>(message)?,
            )
            .await
            .map_err(Refused::of)
    }
}
