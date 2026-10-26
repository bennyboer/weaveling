use appearances_core::{AppearancesCore, Place, SceneLink};

use crate::projects::project_of;
use messaging::{Failure, Handler, Message};
use scenes_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, SceneDeletedDTO};

pub struct ProjectSceneAppearancesListener {
    appearances: AppearancesCore,
}

impl ProjectSceneAppearancesListener {
    pub fn new(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }

    async fn index(&self, message: &Message) -> Result<(), Failure> {
        match message.routing.to_string().as_str() {
            IDEA_LINKED => {
                let link: IdeaLinkDTO = message.payload_as()?;
                self.appearances
                    .appeared(
                        &link.idea,
                        &a_scene(link.scene),
                        link.version,
                        project_of(message).as_deref(),
                    )
                    .await?;
            }
            IDEA_UNLINKED => {
                let link: IdeaLinkDTO = message.payload_as()?;
                self.appearances
                    .disappeared(&link.idea, &a_scene(link.scene), link.version)
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
                self.appearances
                    .place_gone(&a_scene(deleted.scene), message.occurred_at)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
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
