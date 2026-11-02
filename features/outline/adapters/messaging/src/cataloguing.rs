use async_trait::async_trait;
use inbox::{KeptHandler, Refused};
use messaging::Message;
use outline_contract::{DISCARDED, STARTED};
use outline_core::{OutlineId, OutlinesCore};
use transacting::Transaction;

pub struct CatalogueOutlinesListener {
    outlines: OutlinesCore,
}

impl CatalogueOutlinesListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait]
impl KeptHandler for CatalogueOutlinesListener {
    const NAME: &'static str = "catalogue-outline";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, DISCARDED];
    const WHEN_REFUSED: &'static str = "An outline did not reach the list of outlines.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        self.outlines
            .catalogue(
                Some(transaction),
                &eventpublishing::aggregate_in::<OutlineId>(message)?,
            )
            .await
            .map_err(Refused::of)
    }
}
