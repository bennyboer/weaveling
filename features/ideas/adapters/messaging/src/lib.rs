mod broadcasting;
mod publishing;
mod sweeping;

pub use broadcasting::{BroadcastIdeaChangesListener, LIVE};
pub use publishing::message_for;
pub use sweeping::{AT_MOST, DiscardOnProjectDeletedListener};

use async_trait::async_trait;
use ideas_contract::EVERY_IDEA;
use ideas_core::{IdeaId, IdeasCore};
use inbox::{KeptHandler, Refused};
use messaging::Message;
use transacting::Transaction;

pub struct CatalogueIdeasListener {
    ideas: IdeasCore,
}

impl CatalogueIdeasListener {
    pub fn new(ideas: IdeasCore) -> Self {
        Self { ideas }
    }
}

#[async_trait]
impl KeptHandler for CatalogueIdeasListener {
    const NAME: &'static str = "catalogue-idea";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_IDEA];
    const WHEN_REFUSED: &'static str = "A change to an idea did not reach the list of ideas.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        self.ideas
            .catalogue(
                Some(transaction),
                &eventpublishing::aggregate_in::<IdeaId>(message)?,
            )
            .await
            .map_err(Refused::of)
    }
}
