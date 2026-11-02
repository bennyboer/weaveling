use inbox::{KeptHandler, Refused};
use messaging::Message;
use scenes_contract::{
    CREATED, DELETED, OPENING_CHANGED, OpeningChangedDTO, RETITLED, SceneCreatedDTO,
    SceneDeletedDTO, SceneRetitledDTO,
};
use scenes_core::ScenesCore;
use transacting::Transaction;

pub struct CatalogueScenesListener {
    scenes: ScenesCore,
}

impl CatalogueScenesListener {
    pub fn new(scenes: ScenesCore) -> Self {
        Self { scenes }
    }

    async fn catalogue(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        match message.routing.to_string().as_str() {
            CREATED => {
                let created: SceneCreatedDTO = message.payload_as()?;
                self.scenes
                    .catalogue_created(Some(transaction), &created.scene, &created.project)
                    .await?;
            }
            RETITLED => {
                let retitled: SceneRetitledDTO = message.payload_as()?;
                self.scenes
                    .catalogue_retitled(
                        Some(transaction),
                        &retitled.scene,
                        &retitled.project,
                        &retitled.title,
                        retitled.version,
                    )
                    .await?;
            }
            OPENING_CHANGED => {
                let opened: OpeningChangedDTO = message.payload_as()?;
                self.scenes
                    .catalogue_opening(
                        Some(transaction),
                        &opened.scene,
                        &opened.project,
                        &opened.opening,
                        message.occurred_at,
                    )
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
                self.scenes
                    .catalogue_deleted(Some(transaction), &deleted.scene, message.occurred_at)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl KeptHandler for CatalogueScenesListener {
    const NAME: &'static str = "catalogue-scene";
    const LISTENS_TO: &'static [&'static str] = &[CREATED, RETITLED, OPENING_CHANGED, DELETED];
    const WHEN_REFUSED: &'static str = "A scene's name in the outline was not updated.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        self.catalogue(transaction, message).await
    }
}
