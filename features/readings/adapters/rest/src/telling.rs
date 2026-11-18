use std::sync::Arc;

use live::{Broadcaster, Teller};
use readings_contract::BookChangeDTO;
use readings_core::{BookFollowers, OutlineCatalog, ProjectLink, SceneLink, SceneTextCatalog};
use transacting::Transaction;

use crate::BookChanges;
use crate::transformer;

/// Tells whoever follows a project's live book each piece that was kept, once it is committed.
/// The piece is read back from the catalog then, so a telling read before a newer change is told before it.
pub struct TellingFollowers {
    outlines: Arc<dyn OutlineCatalog>,
    texts: Arc<dyn SceneTextCatalog>,
    teller: Arc<Teller<BookChanges>>,
}

impl TellingFollowers {
    pub fn new(
        outlines: Arc<dyn OutlineCatalog>,
        texts: Arc<dyn SceneTextCatalog>,
        broadcaster: Broadcaster<BookChanges>,
    ) -> Self {
        Self {
            outlines,
            texts,
            teller: Arc::new(Teller::new(broadcaster)),
        }
    }
}

impl BookFollowers for TellingFollowers {
    fn text_kept(&self, transaction: &mut Transaction, project: &ProjectLink, scene: &SceneLink) {
        let (texts, teller) = (self.texts.clone(), self.teller.clone());
        let (project, scene) = (project.clone(), scene.clone());

        transaction.after_commit(async move {
            teller
                .tell_latest(BookChanges(project.clone()), project.as_str(), async {
                    let mut kept = texts.texts_of(std::slice::from_ref(&scene)).await.ok()?;

                    kept.remove(&scene).map(|blocks| BookChangeDTO::Text {
                        scene: scene.to_string(),
                        blocks: transformer::to_blocks_dto(&blocks),
                    })
                })
                .await;
        });
    }

    fn outline_kept(&self, transaction: &mut Transaction, project: &ProjectLink) {
        let (outlines, teller, project) =
            (self.outlines.clone(), self.teller.clone(), project.clone());

        transaction.after_commit(async move {
            teller
                .tell_latest(BookChanges(project.clone()), project.as_str(), async {
                    outlines
                        .outline_of(&project)
                        .await
                        .ok()
                        .flatten()
                        .map(|book| BookChangeDTO::Outline {
                            sections: book
                                .sections
                                .iter()
                                .map(transformer::to_outlined_section_dto)
                                .collect(),
                        })
                })
                .await;
        });
    }

    fn title_kept(&self, transaction: &mut Transaction, project: &ProjectLink) {
        let (outlines, teller, project) =
            (self.outlines.clone(), self.teller.clone(), project.clone());

        transaction.after_commit(async move {
            teller
                .tell_latest(BookChanges(project.clone()), project.as_str(), async {
                    outlines
                        .outline_of(&project)
                        .await
                        .ok()
                        .flatten()
                        .map(|book| BookChangeDTO::Title { title: book.title })
                })
                .await;
        });
    }
}
