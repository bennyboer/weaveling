use std::sync::Arc;

use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use scenes_contract::{DELETED, SceneDeletedDTO, WORDS_COUNTED, WordsCountedDTO};
use serde_json::from_value;
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCountCatalog};
use thiserror::Error;

const NAME: &str = "keep-word-counts";

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
impl Listener for WordCountsProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the word counts listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        [WORDS_COUNTED, DELETED]
            .into_iter()
            .map(|key| Subscription::parse(key).expect("a declared routing key holds no wildcards"))
            .collect()
    }

    fn when_refused(&self) -> &'static str {
        "A scene's word count was not updated."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.keep(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
