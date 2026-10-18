use std::sync::Arc;

use messaging::{Failure, Handler, Message};
use scenes_contract::{DELETED, SceneDeletedDTO, WORDS_COUNTED, WordsCountedDTO};
use serde_json::from_value;
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCountCatalog};
use thiserror::Error;

pub struct WordCountsProjector {
    catalog: Arc<dyn WordCountCatalog>,
}

#[derive(Debug, Error)]
enum NotKept {
    #[error("this message is not shaped like a scene announcement: {0}")]
    Unshaped(#[from] serde_json::Error),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl WordCountsProjector {
    pub fn new(catalog: Arc<dyn WordCountCatalog>) -> Self {
        Self { catalog }
    }

    async fn keep(&self, message: &Message) -> Result<(), NotKept> {
        let payload = message.payload.clone();

        match message.routing.to_string().as_str() {
            WORDS_COUNTED => {
                let counted: WordsCountedDTO = from_value(payload)?;
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
                let deleted: SceneDeletedDTO = from_value(payload)?;
                self.catalog
                    .forget_scene(&SceneLink::from(deleted.scene))
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler for WordCountsProjector {
    const NAME: &'static str = "keep-word-counts";
    const LISTENS_TO: &'static [&'static str] = &[WORDS_COUNTED, DELETED];
    const WHEN_REFUSED: &'static str = "A scene's word count was not updated.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self.keep(message).await?)
    }
}
