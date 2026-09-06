use std::fmt::{self, Display, Formatter};

use crate::pieces::model::PieceId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OutlineId(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectionId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: SectionId,
    pub parent: Option<SectionId>,
    pub title: String,
    pub pieces: Vec<PieceId>,
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

impl Outline {
    pub fn holds(&self, piece: &PieceId) -> bool {
        self.sections
            .iter()
            .any(|section| section.pieces.contains(piece))
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
