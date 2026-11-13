use async_trait::async_trait;
use inbox::KeptHandler;
use messaging::{Failure, Message};
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
    const WHEN_REFUSED: &'static str =
        "A new or dismissed notice did not reach the account's list of notices.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        Ok(self
            .notifications
            .catalogue(
                Some(transaction),
                &eventpublishing::aggregate_in::<NoticeId>(message)?,
            )
            .await?)
    }
}
