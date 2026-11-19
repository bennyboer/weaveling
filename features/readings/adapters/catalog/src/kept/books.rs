use readings_core::{CatalogError, OutlinedSection, SceneLink, SectionLink};
use serde::{Deserialize, Serialize};

/// A section as the databases keep it, in the catalog's own form like the blocks.
#[derive(Debug, Serialize, Deserialize)]
struct KeptSection {
    section: String,
    depth: u8,
    title: String,
    scenes: Vec<String>,
}

pub(crate) fn to_json(sections: &[OutlinedSection]) -> String {
    let kept: Vec<KeptSection> = sections
        .iter()
        .map(|outlined| KeptSection {
            section: outlined.section.to_string(),
            depth: outlined.depth,
            title: outlined.title.clone(),
            scenes: outlined.scenes.iter().map(SceneLink::to_string).collect(),
        })
        .collect();

    serde_json::to_string(&kept).expect("sections are plain data and cannot fail to serialize")
}

pub(crate) fn from_json(kept: &str) -> Result<Vec<OutlinedSection>, CatalogError> {
    let kept: Vec<KeptSection> =
        serde_json::from_str(kept).map_err(|why| CatalogError::Backend(Box::new(why)))?;

    Ok(kept
        .into_iter()
        .map(|kept| OutlinedSection {
            section: SectionLink::from(kept.section),
            depth: kept.depth,
            title: kept.title,
            scenes: kept.scenes.into_iter().map(SceneLink::from).collect(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_come_back_as_they_were_kept() {
        // given: a nested section with scenes, and one without
        let sections = vec![
            OutlinedSection {
                section: SectionLink::from("section_market"),
                depth: 1,
                title: "The market".to_owned(),
                scenes: vec![
                    SceneLink::from("scene_stalls"),
                    SceneLink::from("scene_fish"),
                ],
            },
            OutlinedSection {
                section: SectionLink::from("section_evening"),
                depth: 0,
                title: String::new(),
                scenes: vec![],
            },
        ];

        // when: they are written as kept and read back
        let read = from_json(&to_json(&sections)).expect("what was written reads back");

        // then: they are as they were
        assert_eq!(read, sections);
    }
}
