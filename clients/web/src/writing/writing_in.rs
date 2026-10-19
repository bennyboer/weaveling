use leptos::prelude::*;

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::writing::structure::Structure;

#[derive(Clone, Copy)]
pub struct WritingIn {
    structure: Structure,
    section: RwSignal<Option<SectionId>>,
    scene: StoredValue<Option<SceneId>>,
}

impl WritingIn {
    pub fn of(structure: Structure) -> Self {
        Self {
            structure,
            section: RwSignal::new(None),
            scene: StoredValue::new(None),
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

    pub fn wrote_in(self, scene: Option<SceneId>) {
        self.scene.set_value(scene);
    }

    pub fn was_writing_in(self) -> Option<(SectionId, Option<SceneId>)> {
        let section = self.section.get_untracked()?;

        Some((section, self.scene.get_value()))
    }

    pub fn last_written_in(self) -> Option<SectionId> {
        self.section
            .get()
            .filter(|section| self.structure.title_of(section).is_some())
    }

    pub fn section(self) -> Option<SectionId> {
        self.section
            .get()
            .filter(|section| self.structure.title_of(section).is_some())
            .or_else(|| self.structure.first_section())
    }
}
