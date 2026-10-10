use std::sync::Arc;

use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, PassageLink, Place, Subject};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use passages_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, PassageDeletedDTO};
use serde_json::from_value;
use thiserror::Error;

const NAME: &str = "index-passage-appearances";

pub struct PassageAppearancesProjector {
    catalog: Arc<dyn AppearanceCatalog>,
}

#[derive(Debug, Error)]
enum NotIndexed {
    #[error("this message is not shaped like a passage announcement: {0}")]
    Unshaped(#[from] serde_json::Error),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl PassageAppearancesProjector {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
    }

    async fn index(&self, message: &Message) -> Result<(), NotIndexed> {
        let payload = message.payload.clone();

        match message.routing.to_string().as_str() {
            IDEA_LINKED => {
                let link: IdeaLinkDTO = from_value(payload)?;
                self.catalog
                    .remember(&an_idea(link.idea), &a_passage(link.passage), link.version)
                    .await?;
            }
            IDEA_UNLINKED => {
                let link: IdeaLinkDTO = from_value(payload)?;
                self.catalog
                    .forget(&an_idea(link.idea), &a_passage(link.passage), link.version)
                    .await?;
            }
            DELETED => {
                let deleted: PassageDeletedDTO = from_value(payload)?;
                self.catalog
                    .forget_place(&a_passage(deleted.passage))
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

fn an_idea(id: String) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

fn a_passage(id: String) -> Place {
    Place::Passage(PassageLink::from(id))
}

#[async_trait::async_trait]
impl Listener for PassageAppearancesProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME)
            .expect("the passage appearances listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        [IDEA_LINKED, IDEA_UNLINKED, DELETED]
            .into_iter()
            .map(|key| Subscription::parse(key).expect("a declared routing key holds no wildcards"))
            .collect()
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.index(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
