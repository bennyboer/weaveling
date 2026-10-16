use leptos::prelude::*;

use crate::outline::model::SectionId;
use crate::writing::structure::Structure;

#[derive(Clone, Copy)]
pub struct WritingIn {
    structure: Structure,
    section: RwSignal<Option<SectionId>>,
}

impl WritingIn {
    pub fn of(structure: Structure) -> Self {
        Self {
            structure,
            section: RwSignal::new(None),
        }
    }

    pub fn moved_to(self, section: SectionId) {
        if self
            .section
            .with_untracked(|now| now.as_ref() != Some(&section))
        {
            self.section.set(Some(section));
        }
    }

    pub fn section(self) -> Option<SectionId> {
        self.section
            .get()
            .filter(|section| self.structure.title_of(section).is_some())
            .or_else(|| self.structure.first_section())
    }
}
