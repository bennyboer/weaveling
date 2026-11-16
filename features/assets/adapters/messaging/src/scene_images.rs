use assets_core::{AssetsCore, ProjectLink, SceneLink};
use async_trait::async_trait;
use eventsourcing::Version;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use projects_contract::context::PROJECT;
use scenes_contract::{ASSETS_CHANGED, DELETED, SceneAssetsChangedDTO, SceneDeletedDTO};
use transacting::Transaction;

/// Records which images each scene holds, so an image is not deleted while a scene shows it.
/// Only the images of the scene's own project are recorded.
/// When a scene is deleted, its images are no longer in use, and no late message can bring the uses back.
pub struct RememberSceneImagesListener {
    assets: AssetsCore,
}

impl RememberSceneImagesListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

#[async_trait]
impl KeptHandler for RememberSceneImagesListener {
    const NAME: &'static str = "assets-remember-scene-images";
    const LISTENS_TO: &'static [&'static str] = &[ASSETS_CHANGED, DELETED];
    const WHEN_REFUSED: &'static str = "Which images a scene shows did not reach the assets feature. An image may still be protected from deletion although no scene shows it, or not be protected although a scene shows it.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        match message.routing.to_string().as_str() {
            ASSETS_CHANGED => {
                let told: SceneAssetsChangedDTO = message.payload_as()?;
                let context = eventpublishing::context_in(message);
                let project = context
                    .get(PROJECT)
                    .ok_or("the images of a scene name no project")?;
                Ok(self
                    .assets
                    .scene_assets_changed(
                        Some(transaction),
                        &SceneLink::from(told.scene.as_str()),
                        &ProjectLink::from(project),
                        &told.assets,
                        Version::of(told.version),
                    )
                    .await?)
            }
            DELETED => {
                let deleted: SceneDeletedDTO = message.payload_as()?;
                Ok(self
                    .assets
                    .scene_deleted(
                        Some(transaction),
                        &SceneLink::from(deleted.scene.as_str()),
                        message.occurred_at,
                    )
                    .await?)
            }
            _ => Err("this tells no change of the images in a scene".into()),
        }
    }
}
