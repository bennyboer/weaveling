use std::sync::Arc;

use messaging::{Failure, Handler, Message};
use scenes_contract::{DELETED, SceneDeletedDTO, WORDS_COUNTED, WordsCountedDTO};
use statistics_core::{ProjectLink, SceneLink, WordCountCatalog};

pub struct ProjectWordCountsListener {
    catalog: Arc<dyn WordCountCatalog>,
}

impl ProjectWordCountsListener {
    pub fn new(catalog: Arc<dyn WordCountCatalog>) -> Self {
        Self { catalog }
    }

    async fn keep(&self, message: &Message) -> Result<(), Failure> {
        match message.routing.to_string().as_str() {
            WORDS_COUNTED => {
                let counted: WordsCountedDTO = message.payload_as()?;
                self.catalog
                    .keep(
                        &SceneLink::from(counted.scene),
                        &ProjectLink::from(counted.project),
                        counted.words,
                        message.occurred_at,
                    )
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
                self.catalog
                    .forget_scene(&SceneLink::from(deleted.scene), message.occurred_at)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler for ProjectWordCountsListener {
    const NAME: &'static str = "keep-word-counts";
    const LISTENS_TO: &'static [&'static str] = &[WORDS_COUNTED, DELETED];
    const WHEN_REFUSED: &'static str = "A scene's word count was not updated.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.keep(message).await
    }
}
