use std::sync::Arc;

use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use scenes_contract::{
    CREATED, DELETED, OPENING_CHANGED, OpeningChangedDTO, RETITLED, SceneCreatedDTO,
    SceneDeletedDTO, SceneRetitledDTO,
};
use scenes_core::{
    CatalogError, InvalidSceneTitle, ProjectLink, SceneCatalog, SceneId, SceneTitle,
};
use serde_json::from_value;
use thiserror::Error;

const NAME: &str = "catalogue-scene";

pub struct CatalogueScenes {
    catalog: Arc<dyn SceneCatalog>,
}

#[derive(Debug, Error)]
enum NotCatalogued {
    #[error("this message is not shaped like a scene announcement: {0}")]
    Unshaped(#[from] serde_json::Error),
    #[error("{0} is not a scene id")]
    NotASceneId(String),
    #[error(transparent)]
    Untitled(#[from] InvalidSceneTitle),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

fn scene_id(raw: &str) -> Result<SceneId, NotCatalogued> {
    raw.parse()
        .map_err(|_| NotCatalogued::NotASceneId(raw.to_owned()))
}

impl CatalogueScenes {
    pub fn new(catalog: Arc<dyn SceneCatalog>) -> Self {
        Self { catalog }
    }

    async fn catalogue(&self, message: &Message) -> Result<(), NotCatalogued> {
        let payload = message.payload.clone();

        match message.routing.to_string().as_str() {
            CREATED => {
                let created: SceneCreatedDTO = from_value(payload)?;
                self.catalog
                    .remember(
                        scene_id(&created.scene)?,
                        &ProjectLink::from(created.project),
                    )
                    .await?;
            }
            RETITLED => {
                let retitled: SceneRetitledDTO = from_value(payload)?;
                self.catalog
                    .retitle(
                        scene_id(&retitled.scene)?,
                        &ProjectLink::from(retitled.project),
                        &SceneTitle::new(&retitled.title)?,
                        retitled.version,
                    )
                    .await?;
            }
            OPENING_CHANGED => {
                let opened: OpeningChangedDTO = from_value(payload)?;
                self.catalog
                    .note_opening(
                        scene_id(&opened.scene)?,
                        &ProjectLink::from(opened.project),
                        &opened.opening,
                        message.occurred_at,
                    )
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = from_value(payload)?;
                self.catalog.forget(scene_id(&deleted.scene)?).await?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Listener for CatalogueScenes {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the catalogue listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        [CREATED, RETITLED, OPENING_CHANGED, DELETED]
            .into_iter()
            .map(|key| Subscription::parse(key).expect("a declared routing key holds no wildcards"))
            .collect()
    }

    fn when_refused(&self) -> &'static str {
        "A scene's name in the outline was not updated."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.catalogue(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
