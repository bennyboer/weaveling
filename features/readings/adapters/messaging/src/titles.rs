use inbox::KeptHandler;
use messaging::{Failure, Message};
use projects_contract::{DELETED, ProjectEventDTO, RENAMED, STARTED};
use readings_core::ReadingsCore;
use transacting::Transaction;

pub struct KeepBookTitlesListener {
    readings: ReadingsCore,
}

impl KeepBookTitlesListener {
    pub fn new(readings: ReadingsCore) -> Self {
        Self { readings }
    }
}

#[async_trait::async_trait]
impl KeptHandler for KeepBookTitlesListener {
    const NAME: &'static str = "readings-keep-book-titles";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, RENAMED, DELETED];
    const WHEN_REFUSED: &'static str = "A project's name or its deletion did not reach the readings feature. The book may show the project's old name, or stay there after the project was deleted.";

    /// Keeps the project's name as its book's title, or forgets the book once the project is deleted.
    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = told.aggregate.id.as_str();

        match told.event.body {
            ProjectEventDTO::Started { name } | ProjectEventDTO::Renamed { name } => Ok(self
                .readings
                .title_changed(Some(transaction), project, &name, told.aggregate.version)
                .await?),
            ProjectEventDTO::Deleted => Ok(self
                .readings
                .book_gone(Some(transaction), project, message.occurred_at)
                .await?),
            ProjectEventDTO::DetailsChanged { .. }
            | ProjectEventDTO::QuotationChanged { .. }
            | ProjectEventDTO::Trashed { .. }
            | ProjectEventDTO::Restored => {
                Err("this tells neither a project's name nor that it was deleted".into())
            }
        }
    }
}
