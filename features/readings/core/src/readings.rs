use std::collections::HashMap;
use std::sync::Arc;

use ::access::KeptAccess;
use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::Agent;
use time::OffsetDateTime;
use transacting::Transactions;

use crate::catalog::{BookOutline, OutlineCatalog, SceneTextCatalog};
use crate::contents::{Block, Contents, Part};
use crate::error::ReadingsError;
use crate::link::{ProjectLink, SceneLink};

mod access;
mod outlines;
mod texts;

pub use outlines::ToldSection;

#[derive(Clone)]
pub struct ReadingsCore {
    outlines: Arc<dyn OutlineCatalog>,
    texts: Arc<dyn SceneTextCatalog>,
    transactions: Arc<dyn Transactions>,
    access: KeptAccess,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl ReadingsCore {
    pub fn new(
        outlines: Arc<dyn OutlineCatalog>,
        texts: Arc<dyn SceneTextCatalog>,
        transactions: Arc<dyn Transactions>,
        access: KeptAccess,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            outlines,
            texts,
            transactions,
            access,
            authorizer,
            clock,
        }
    }

    /// The book as it is written now, put together from its outline and its scenes' text.
    /// It is there only for whoever may read the project.
    pub async fn live_book(&self, project: &str, agent: &Agent) -> Result<Contents, ReadingsError> {
        if !self.authorizer.may(agent, Act::Read, project).await? {
            return Err(ReadingsError::ProjectNotThere);
        }
        let project = ProjectLink::from(project);
        let Some(outline) = self.outlines.outline_of(&project).await? else {
            return Err(ReadingsError::ProjectNotThere);
        };
        let scenes = outline
            .sections
            .iter()
            .flat_map(|section| section.scenes.iter().cloned())
            .collect::<Vec<_>>();
        let texts = self.texts.texts_of(&scenes).await?;

        Ok(put_together(project, outline, texts))
    }
}

impl ReadingsCore {
    /// Forgets, in batches, the tombstones laid long enough ago that no message about them can still arrive:
    /// those of deleted scenes, those of deleted projects' books, and those of accounts that left a project.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, ReadingsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let texts = self
                .texts
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let books = self
                .outlines
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let access = self
                .access
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(texts + books + access)
        })
    }
}

/// Lays the book out in reading order: each section, then the scenes it holds.
/// A scene whose text is not kept yet is left out, rather than shown empty.
fn put_together(
    project: ProjectLink,
    outline: BookOutline,
    mut texts: HashMap<SceneLink, Vec<Block>>,
) -> Contents {
    let mut parts = Vec::new();
    for section in outline.sections {
        parts.push(Part::Section {
            section: section.section,
            depth: section.depth,
            title: section.title,
        });
        for scene in section.scenes {
            if let Some(blocks) = texts.remove(&scene) {
                parts.push(Part::Scene { scene, blocks });
            }
        }
    }

    Contents {
        project,
        title: outline.title,
        parts,
    }
}
