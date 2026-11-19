use inbox::KeptHandler;
use messaging::{Failure, Message};
use readings_contract::{DELETED, EXTENDED, MADE, RENAMED};
use readings_core::{ReadingCopyId, ReadingsCore};
use transacting::Transaction;

pub struct CatalogueReadingCopiesListener {
    readings: ReadingsCore,
}

impl CatalogueReadingCopiesListener {
    pub fn new(readings: ReadingsCore) -> Self {
        Self { readings }
    }
}

#[async_trait::async_trait]
impl KeptHandler for CatalogueReadingCopiesListener {
    const NAME: &'static str = "catalogue-reading-copy";
    const LISTENS_TO: &'static [&'static str] = &[MADE, RENAMED, EXTENDED, DELETED];
    const WHEN_REFUSED: &'static str = "A reading copy did not reach the list of a project's copies. It may be missing from the list, show an old name or old dates, or stay listed after it was deleted.";

    /// Keeps the copy in its project's list as it now stands, or forgets it once it is deleted.
    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        Ok(self
            .readings
            .catalogue(
                Some(transaction),
                &eventpublishing::aggregate_in::<ReadingCopyId>(message)?,
            )
            .await?)
    }
}
