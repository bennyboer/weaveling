use std::collections::HashMap;
use std::sync::Arc;

use eventsourcing::Agent;

use crate::catalog::{BookOutline, OutlineCatalog, SceneTextCatalog};
use crate::contents::{Block, Contents, Part};
use crate::error::ReadingsError;
use crate::link::{ProjectLink, SceneLink};

#[derive(Clone)]
pub struct ReadingsCore {
    outlines: Arc<dyn OutlineCatalog>,
    texts: Arc<dyn SceneTextCatalog>,
}

impl ReadingsCore {
    pub fn new(outlines: Arc<dyn OutlineCatalog>, texts: Arc<dyn SceneTextCatalog>) -> Self {
        Self { outlines, texts }
    }

    /// The book as it is written now, put together from its outline and its scenes' text.
    pub async fn live_book(
        &self,
        project: &str,
        _agent: &Agent,
    ) -> Result<Contents, ReadingsError> {
        // TODO: members only
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
