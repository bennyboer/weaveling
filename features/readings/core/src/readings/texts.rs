use time::OffsetDateTime;
use transacting::Transaction;

use crate::contents::Block;
use crate::error::ReadingsError;
use crate::link::{ProjectLink, SceneLink};
use crate::readings::ReadingsCore;

impl ReadingsCore {
    /// Keeps a scene's text as the scenes feature told it once it had settled, read at its prose version.
    /// Whoever follows the book of the scene's project is told the new text, if the scenes feature named the project.
    pub async fn text_changed(
        &self,
        within: Option<&mut Transaction>,
        project: Option<&str>,
        scene: &str,
        blocks: &[Block],
        version: u64,
    ) -> Result<(), ReadingsError> {
        let scene = SceneLink::from(scene);

        transacting::joining!(&*self.transactions, within, |transaction| {
            let kept = self
                .texts
                .keep(transaction, &scene, blocks, version)
                .await?;
            if let (true, Some(project)) = (kept, project) {
                self.followers
                    .text_kept(transaction, &ProjectLink::from(project), &scene);
            }

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
