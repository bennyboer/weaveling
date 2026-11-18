use std::collections::HashMap;

use time::OffsetDateTime;
use transacting::Transaction;

use crate::catalog::OutlinedSection;
use crate::error::ReadingsError;
use crate::link::{ProjectLink, SceneLink, SectionLink};
use crate::readings::ReadingsCore;

/// A section as the outline tells it: under which section it sits, rather than how deep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToldSection {
    pub section: SectionLink,
    pub parent: Option<SectionLink>,
    pub title: String,
    pub scenes: Vec<SceneLink>,
}

impl ReadingsCore {
    /// Keeps a book's sections as the outline told them at a version.
    /// The sections come in reading order, each after its parent.
    pub async fn outline_changed(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        sections: &[ToldSection],
        version: u64,
    ) -> Result<(), ReadingsError> {
        let outlined = nested(sections);
        let project = ProjectLink::from(project);

        transacting::joining!(&*self.transactions, within, |transaction| {
            if self
                .outlines
                .keep_outline(transaction, &project, &outlined, version)
                .await?
            {
                self.followers.outline_kept(transaction, &project);
            }

            Ok(())
        })
    }

    /// Keeps a book's title as the projects feature named the project at a version.
    pub async fn title_changed(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        title: &str,
        version: u64,
    ) -> Result<(), ReadingsError> {
        let project = ProjectLink::from(project);

        transacting::joining!(&*self.transactions, within, |transaction| {
            if self
                .outlines
                .keep_title(transaction, &project, title, version)
                .await?
            {
                self.followers.title_kept(transaction, &project);
            }

            Ok(())
        })
    }

    /// Forgets the book of a project that was deleted, or whose outline was discarded.
    pub async fn book_gone(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        at: OffsetDateTime,
    ) -> Result<(), ReadingsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.outlines
                .forget_book(transaction, &ProjectLink::from(project), at)
                .await?;

            Ok(())
        })
    }
}

/// Each section with how deep it is nested, counted from its parent's depth.
/// A section whose parent is not among those before it is taken for one at the top.
fn nested(sections: &[ToldSection]) -> Vec<OutlinedSection> {
    let mut depths: HashMap<&SectionLink, u8> = HashMap::with_capacity(sections.len());

    sections
        .iter()
        .map(|told| {
            let depth = told
                .parent
                .as_ref()
                .and_then(|parent| depths.get(parent))
                .map_or(0, |above| above.saturating_add(1));
            depths.insert(&told.section, depth);

            OutlinedSection {
                section: told.section.clone(),
                depth,
                title: told.title.clone(),
                scenes: told.scenes.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn told(section: &str, parent: Option<&str>) -> ToldSection {
        ToldSection {
            section: SectionLink::from(section),
            parent: parent.map(SectionLink::from),
            title: section.to_owned(),
            scenes: vec![],
        }
    }

    #[test]
    fn a_section_lies_one_deeper_than_its_parent() {
        // given: two chapters, a part under the first, and a scene group under the part
        let sections = [
            told("first", None),
            told("part", Some("first")),
            told("group", Some("part")),
            told("second", None),
        ];

        // when: they are nested
        let depths: Vec<u8> = nested(&sections)
            .into_iter()
            .map(|section| section.depth)
            .collect();

        // then: each lies one deeper than its parent
        assert_eq!(depths, vec![0, 1, 2, 0]);
    }

    #[test]
    fn a_section_whose_parent_was_not_told_lies_at_the_top() {
        // when: a section names a parent that is not among the sections
        let outlined = nested(&[told("lost", Some("nowhere"))]);

        // then: it lies at the top, rather than being left out
        assert_eq!(outlined[0].depth, 0);
    }
}
