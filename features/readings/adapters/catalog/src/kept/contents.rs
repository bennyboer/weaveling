use readings_core::{CatalogError, Contents, Part, ProjectLink, SceneLink, SectionLink};
use serde::{Deserialize, Serialize};

use crate::kept::blocks::{self, KeptBlock};

/// What a reading copy holds, as the databases keep it, in the catalog's own form like the blocks.
/// The project is kept in a column of its own beside it.
#[derive(Debug, Serialize, Deserialize)]
struct KeptContents {
    title: String,
    parts: Vec<KeptPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum KeptPart {
    Section {
        section: String,
        depth: u8,
        title: String,
    },
    Scene {
        scene: String,
        blocks: Vec<KeptBlock>,
    },
}

pub(crate) fn to_json(contents: &Contents) -> String {
    let kept = KeptContents {
        title: contents.title.clone(),
        parts: contents
            .parts
            .iter()
            .map(|part| match part {
                Part::Section {
                    section,
                    depth,
                    title,
                } => KeptPart::Section {
                    section: section.to_string(),
                    depth: *depth,
                    title: title.clone(),
                },
                Part::Scene { scene, blocks } => KeptPart::Scene {
                    scene: scene.to_string(),
                    blocks: blocks.iter().map(blocks::kept_as).collect(),
                },
            })
            .collect(),
    };

    serde_json::to_string(&kept).expect("contents are plain data and cannot fail to serialize")
}

pub(crate) fn from_json(project: &str, kept: &str) -> Result<Contents, CatalogError> {
    let kept: KeptContents =
        serde_json::from_str(kept).map_err(|why| CatalogError::Backend(Box::new(why)))?;

    Ok(Contents {
        project: ProjectLink::from(project),
        title: kept.title,
        parts: kept
            .parts
            .into_iter()
            .map(|part| match part {
                KeptPart::Section {
                    section,
                    depth,
                    title,
                } => Part::Section {
                    section: SectionLink::from(section),
                    depth,
                    title,
                },
                KeptPart::Scene { scene, blocks } => Part::Scene {
                    scene: SceneLink::from(scene),
                    blocks: blocks.into_iter().map(blocks::block_of).collect(),
                },
            })
            .collect(),
    })
}
