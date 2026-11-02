use inbox::KeptHandler;
use messaging::{Failure, Message};
use scenes_contract::{DELETED, SceneDeletedDTO, WORDS_COUNTED, WordsCountedDTO};
use statistics_core::StatisticsCore;
use transacting::Transaction;

pub struct ProjectWordCountsListener {
    statistics: StatisticsCore,
}

impl ProjectWordCountsListener {
    pub fn new(statistics: StatisticsCore) -> Self {
        Self { statistics }
    }

    async fn keep(&self, transaction: &mut Transaction, message: &Message) -> Result<(), Failure> {
        match message.routing.to_string().as_str() {
            WORDS_COUNTED => {
                let counted: WordsCountedDTO = message.payload_as()?;
                self.statistics
                    .counted(
                        Some(transaction),
                        &counted.scene,
                        &counted.project,
                        counted.words,
                        message.occurred_at,
                    )
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
                self.statistics
                    .scene_deleted(Some(transaction), &deleted.scene, message.occurred_at)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl KeptHandler for ProjectWordCountsListener {
    const NAME: &'static str = "keep-word-counts";
    const LISTENS_TO: &'static [&'static str] = &[WORDS_COUNTED, DELETED];
    const WHEN_REFUSED: &'static str = "A scene's word count was not updated.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        self.keep(transaction, message).await
    }
}
