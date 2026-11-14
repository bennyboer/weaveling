use accounts_contract::{AccountEventDTO, PICTURE_CHANGED, PICTURE_REMOVED};
use assets_core::AssetsCore;
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use messaging::{Failure, Message};
use transacting::Transaction;

/// Records which image each account shows as its picture, so that image is not deleted while in use.
/// When the account asks for it, the picture shown before is deleted, if nothing else uses it.
pub struct RememberPicturesListener {
    assets: AssetsCore,
}

impl RememberPicturesListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

#[async_trait]
impl KeptHandler for RememberPicturesListener {
    const NAME: &'static str = "assets-remember-pictures";
    const LISTENS_TO: &'static [&'static str] = &[PICTURE_CHANGED, PICTURE_REMOVED];
    const WHEN_REFUSED: &'static str = "An account's new picture, or its removal, did not reach the assets feature. The image may still be protected from deletion, or not be deleted although that was asked for.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let account = AccountId::from(told.aggregate.id);
        let version = Version::of(told.aggregate.version);

        match told.event.body {
            AccountEventDTO::PictureChanged {
                picture,
                previous,
                drop_previous,
            } => Ok(self
                .assets
                .picture_changed(
                    Some(transaction),
                    &account,
                    version,
                    &picture,
                    previous.as_deref(),
                    drop_previous,
                )
                .await?),
            AccountEventDTO::PictureRemoved { picture, drop } => Ok(self
                .assets
                .picture_removed(Some(transaction), &account, version, &picture, drop)
                .await?),
            _ => Err("this tells no change of an account's picture".into()),
        }
    }
}
