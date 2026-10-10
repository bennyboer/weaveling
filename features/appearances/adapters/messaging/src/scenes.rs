use std::sync::Arc;

use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Place, SceneLink, Subject};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use scenes_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, SceneDeletedDTO};
use serde_json::from_value;
use thiserror::Error;

const NAME: &str = "index-scene-appearances";

pub struct SceneAppearancesProjector {
    catalog: Arc<dyn AppearanceCatalog>,
}

#[derive(Debug, Error)]
enum NotIndexed {
    #[error("this message is not shaped like a scene announcement: {0}")]
    Unshaped(#[from] serde_json::Error),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl SceneAppearancesProjector {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
    }

    async fn index(&self, message: &Message) -> Result<(), NotIndexed> {
        let payload = message.payload.clone();

        match message.routing.to_string().as_str() {
            IDEA_LINKED => {
                let link: IdeaLinkDTO = from_value(payload)?;
                self.catalog
                    .remember(&an_idea(link.idea), &a_scene(link.scene), link.version)
                    .await?;
            }
            IDEA_UNLINKED => {
                let link: IdeaLinkDTO = from_value(payload)?;
                self.catalog
                    .forget(&an_idea(link.idea), &a_scene(link.scene), link.version)
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = from_value(payload)?;
                self.catalog.forget_place(&a_scene(deleted.scene)).await?;
            }
            _ => {}
        }

        Ok(())
    }
}

fn an_idea(id: String) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

fn a_scene(id: String) -> Place {
    Place::Scene(SceneLink::from(id))
}

#[async_trait::async_trait]
impl Listener for SceneAppearancesProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the scene appearances listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        [IDEA_LINKED, IDEA_UNLINKED, DELETED]
            .into_iter()
            .map(|key| Subscription::parse(key).expect("a declared routing key holds no wildcards"))
            .collect()
    }

    fn when_refused(&self) -> &'static str {
        "Where an idea appears in your scenes was not updated."
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
