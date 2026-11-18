use inbox::KeptHandler;
use messaging::{Failure, Message};
use readings_core::ReadingsCore;
use scenes_contract::{DELETED, SceneDeletedDTO, SceneTextSettledDTO, TEXT_SETTLED};
use transacting::Transaction;

use crate::transformer;

pub struct KeepSceneTextsListener {
    readings: ReadingsCore,
}

impl KeepSceneTextsListener {
    pub fn new(readings: ReadingsCore) -> Self {
        Self { readings }
    }

    async fn keep(&self, transaction: &mut Transaction, message: &Message) -> Result<(), Failure> {
        match message.routing.to_string().as_str() {
            TEXT_SETTLED => {
                let settled: SceneTextSettledDTO = message.payload_as()?;
                self.readings
                    .text_settled(
                        Some(transaction),
                        &settled.scene,
                        &transformer::to_blocks(&settled.blocks),
                        settled.version,
                    )
                    .await?;
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
                self.readings
                    .scene_deleted(Some(transaction), &deleted.scene, message.occurred_at)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl KeptHandler for KeepSceneTextsListener {
    const NAME: &'static str = "readings-keep-scene-texts";
    const LISTENS_TO: &'static [&'static str] = &[TEXT_SETTLED, DELETED];
    const WHEN_REFUSED: &'static str =
        "A scene's text did not reach the readings feature. The book may show it as it was before.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        self.keep(transaction, message).await
    }
}
