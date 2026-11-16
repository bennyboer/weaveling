use appearances_core::AppearancesCore;
use assets_contract::{AssetEventDTO, DELETED, OwnerDTO, UPLOADED};
use inbox::KeptHandler;
use messaging::{Failure, Message};
use transacting::Transaction;

/// Learns which project an asset belongs to when it is uploaded, and forgets a deleted asset everywhere it appeared.
pub struct AssetAppearancesListener {
    appearances: AppearancesCore,
}

impl AssetAppearancesListener {
    pub fn new(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }
}

#[async_trait::async_trait]
impl KeptHandler for AssetAppearancesListener {
    const NAME: &'static str = "follow-assets-for-appearances";
    const LISTENS_TO: &'static [&'static str] = &[UPLOADED, DELETED];
    const WHEN_REFUSED: &'static str = "The appearances feature did not learn of an uploaded or deleted asset. So where it appears may be shown wrongly.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let published = eventpublishing::published_in::<AssetEventDTO>(message)?;
        let asset = &published.aggregate.id;

        match published.event.body {
            AssetEventDTO::Uploaded {
                owner: OwnerDTO::Project(project),
                ..
            } => {
                self.appearances
                    .asset_uploaded(Some(transaction), asset, &project)
                    .await?
            }
            AssetEventDTO::Uploaded { .. } => {}
            AssetEventDTO::Deleted => {
                self.appearances
                    .asset_deleted(Some(transaction), asset, message.occurred_at)
                    .await?
            }
        }

        Ok(())
    }
}
