use appearances_core::{AppearancesCore, Place, SceneLink};

use crate::projects::project_of;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use scenes_contract::{
    ASSETS_CHANGED, DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, SceneAssetsChangedDTO,
    SceneDeletedDTO,
};
use transacting::Transaction;

pub struct ProjectSceneAppearancesListener {
    appearances: AppearancesCore,
}

impl ProjectSceneAppearancesListener {
    pub fn new(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }

    async fn index(&self, transaction: &mut Transaction, message: &Message) -> Result<(), Failure> {
        match message.routing.to_string().as_str() {
            IDEA_LINKED => {
                let link: IdeaLinkDTO = message.payload_as()?;
                self.appearances
                    .appeared(
                        Some(transaction),
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
                    .disappeared(
                        Some(transaction),
                        &link.idea,
                        &a_scene(link.scene),
                        link.version,
                    )
                    .await?;
            }
            ASSETS_CHANGED => {
                let changed: SceneAssetsChangedDTO = message.payload_as()?;
                self.appearances
                    .assets_changed(
                        Some(transaction),
                        &a_scene(changed.scene),
                        &changed.assets,
                        changed.version,
                        project_of(message).as_deref(),
                    )
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
                self.appearances
                    .place_gone(
                        Some(transaction),
                        &a_scene(deleted.scene),
                        message.occurred_at,
                    )
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
impl KeptHandler for ProjectSceneAppearancesListener {
    const NAME: &'static str = "index-scene-appearances";
    const LISTENS_TO: &'static [&'static str] =
        &[IDEA_LINKED, IDEA_UNLINKED, ASSETS_CHANGED, DELETED];
    const WHEN_REFUSED: &'static str =
        "Where an idea or an asset appears in your scenes was not updated.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        self.index(transaction, message).await
    }
}
