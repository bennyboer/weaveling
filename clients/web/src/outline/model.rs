use std::fmt::{self, Display, Formatter};

use crate::ideas::model::IdeaId;
use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OutlineId(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectionId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attachment {
    Scene(SceneId),
    Idea(IdeaId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: SectionId,
    pub parent: Option<SectionId>,
    pub title: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outline {
    pub id: OutlineId,
    pub version: u64,
    pub sections: Vec<Section>,
}

impl From<String> for OutlineId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for OutlineId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl From<String> for SectionId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl Display for SectionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Section {
    pub fn scenes(&self) -> Vec<SceneId> {
        self.attachments
            .iter()
            .filter_map(|attachment| match attachment {
                Attachment::Scene(scene) => Some(scene.clone()),
                Attachment::Idea(_) => None,
            })
            .collect()
    }
}

impl Outline {
    pub fn holds(&self, attachment: &Attachment) -> bool {
        self.sections
            .iter()
            .any(|section| section.attachments.contains(attachment))
    }

    pub fn in_reading_order(&self) -> Vec<(usize, &Section)> {
        let mut read = Vec::with_capacity(self.sections.len());
        self.read_under(None, 0, &mut read);

        read
    }

    fn read_under<'a>(
        &'a self,
        parent: Option<&SectionId>,
        depth: usize,
        read: &mut Vec<(usize, &'a Section)>,
    ) {
        for section in self
            .sections
            .iter()
            .filter(|held| held.parent.as_ref() == parent)
        {
            read.push((depth, section));
            self.read_under(Some(&section.id), depth + 1, read);
        }
    }

    pub fn siblings_of(&self, section: &SectionId) -> Vec<SectionId> {
        let parent = self.parent_of(section);

        self.sections
            .iter()
            .filter(|held| held.parent == parent)
            .map(|held| held.id.clone())
            .collect()
    }

    pub fn rank_of(&self, section: &SectionId) -> Option<usize> {
        self.siblings_of(section)
            .iter()
            .position(|held| held == section)
    }

    pub fn descends_from(&self, section: &SectionId, ancestor: &SectionId) -> bool {
        let mut walking = Some(section.clone());

        while let Some(here) = walking {
            if &here == ancestor {
                return true;
            }

            walking = self.parent_of(&here);
        }

        false
    }

    pub fn last_child_of(&self, section: &SectionId) -> Option<SectionId> {
        self.sections
            .iter()
            .rfind(|held| held.parent.as_ref() == Some(section))
            .map(|held| held.id.clone())
    }

    pub fn before(&self, section: &SectionId) -> Option<SectionId> {
        let siblings = self.siblings_of(section);
        let rank = siblings.iter().position(|held| held == section)?;

        rank.checked_sub(1)
            .and_then(|back| siblings.get(back).cloned())
    }

    pub fn can_promote(&self, section: &SectionId) -> bool {
        self.parent_of(section).is_some()
    }

    pub fn can_demote(&self, section: &SectionId) -> bool {
        self.rank_of(section).is_some_and(|rank| rank > 0)
    }

    pub fn can_move_earlier(&self, section: &SectionId) -> bool {
        self.can_demote(section)
    }

    pub fn can_move_later(&self, section: &SectionId) -> bool {
        let siblings = self.siblings_of(section);

        siblings
            .iter()
            .position(|held| held == section)
            .is_some_and(|rank| rank + 1 < siblings.len())
    }

    pub fn landing_for(&self, section: &SectionId, later: bool) -> Option<SectionId> {
        let siblings = self.siblings_of(section);
        let rank = siblings.iter().position(|held| held == section)?;

        match later {
            true => siblings.get(rank + 1).cloned(),
            false => rank
                .checked_sub(2)
                .and_then(|before| siblings.get(before).cloned()),
        }
    }

    pub fn parent_of(&self, section: &SectionId) -> Option<SectionId> {
        self.sections
            .iter()
            .find(|held| &held.id == section)
            .and_then(|held| held.parent.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_section(id: &str, parent: Option<&str>) -> Section {
        Section {
            id: SectionId::from(id.to_owned()),
            parent: parent.map(|parent| SectionId::from(parent.to_owned())),
            title: id.to_owned(),
            attachments: Vec::new(),
        }
    }

    fn an_outline(sections: Vec<Section>) -> Outline {
        Outline {
            id: OutlineId::from("outline_1".to_owned()),
            version: 1,
            sections,
        }
    }

    fn read(outline: &Outline) -> Vec<(usize, &str)> {
        outline
            .in_reading_order()
            .into_iter()
            .map(|(depth, section)| (depth, section.title.as_str()))
            .collect()
    }

    #[test]
    fn a_book_is_read_depth_first_with_each_section_before_its_children() {
        let outline = an_outline(vec![
            a_section("part_1", None),
            a_section("part_2", None),
            a_section("chapter_1", Some("part_1")),
            a_section("scene_a", Some("chapter_1")),
            a_section("chapter_2", Some("part_1")),
            a_section("chapter_3", Some("part_2")),
        ]);

        assert_eq!(
            read(&outline),
            vec![
                (0, "part_1"),
                (1, "chapter_1"),
                (2, "scene_a"),
                (1, "chapter_2"),
                (0, "part_2"),
                (1, "chapter_3"),
            ],
            "the outline keeps sections flat, so a child stored after the next part is still read inside its own"
        );
    }

    #[test]
    fn an_empty_book_has_nothing_to_read() {
        assert!(an_outline(Vec::new()).in_reading_order().is_empty());
    }
}
