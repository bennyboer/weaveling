use time::OffsetDateTime;
use transacting::Transaction;

use crate::contents::Block;
use crate::error::ReadingsError;
use crate::link::SceneLink;
use crate::readings::ReadingsCore;

impl ReadingsCore {
    /// Keeps a scene's text as the scenes feature told it once it had settled, read at its prose version.
    pub async fn text_changed(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        blocks: &[Block],
        version: u64,
    ) -> Result<(), ReadingsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.texts
                .keep(transaction, &SceneLink::from(scene), blocks, version)
                .await?;

            Ok(())
        })
    }

    /// Forgets the text of a scene that was deleted.
    pub async fn scene_deleted(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        at: OffsetDateTime,
    ) -> Result<(), ReadingsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.texts
                .forget_scene(transaction, &SceneLink::from(scene), at)
                .await?;

            Ok(())
        })
    }
}
