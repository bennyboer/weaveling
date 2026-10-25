use std::sync::Arc;

use appearances_core::{AppearanceCatalog, IdeaLink, Place, SceneLink, Subject};

use crate::projects::remember_project_of;
use messaging::{Failure, Handler, Message};
use scenes_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, SceneDeletedDTO};

pub struct ProjectSceneAppearancesListener {
    catalog: Arc<dyn AppearanceCatalog>,
}

impl ProjectSceneAppearancesListener {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
    }

    async fn index(&self, message: &Message) -> Result<(), Failure> {
        match message.routing.to_string().as_str() {
            IDEA_LINKED => {
                let link: IdeaLinkDTO = message.payload_as()?;
                let idea = an_idea(link.idea);
                remember_project_of(self.catalog.as_ref(), &idea, message).await?;
                self.catalog
                    .remember(&idea, &a_scene(link.scene), link.version)
                    .await?;
            }
            IDEA_UNLINKED => {
                let link: IdeaLinkDTO = message.payload_as()?;
                self.catalog
                    .forget(&an_idea(link.idea), &a_scene(link.scene), link.version)
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
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
impl Handler for ProjectSceneAppearancesListener {
    const NAME: &'static str = "index-scene-appearances";
    const LISTENS_TO: &'static [&'static str] = &[IDEA_LINKED, IDEA_UNLINKED, DELETED];
    const WHEN_REFUSED: &'static str = "Where an idea appears in your scenes was not updated.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.index(message).await
    }
}
