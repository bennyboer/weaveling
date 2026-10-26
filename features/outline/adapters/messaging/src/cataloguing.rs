use async_trait::async_trait;
use messaging::{Failure, Handler, Message};
use outline_contract::{DISCARDED, STARTED};
use outline_core::{OutlineId, OutlinesCore};

pub struct CatalogueOutlinesListener {
    outlines: OutlinesCore,
}

impl CatalogueOutlinesListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait]
impl Handler for CatalogueOutlinesListener {
    const NAME: &'static str = "catalogue-outline";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, DISCARDED];
    const WHEN_REFUSED: &'static str = "An outline did not reach the list of outlines.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .outlines
            .catalogue(&eventpublishing::aggregate_in::<OutlineId>(message)?)
            .await?)
    }
}
