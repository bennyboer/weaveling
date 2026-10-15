use std::collections::HashMap;

use leptos::prelude::*;

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::statistics::model::SceneWordCount;
use crate::writing::structure::Structure;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counted {
    Section,
    Book,
}

#[derive(Clone, Copy)]
pub struct WordCount {
    structure: Structure,
    words: RwSignal<HashMap<SceneId, usize>>,
    writing_in: RwSignal<Option<SectionId>>,
    counted: RwSignal<Counted>,
}

impl WordCount {
    pub fn of(structure: Structure) -> Self {
        Self {
            structure,
            words: RwSignal::new(HashMap::new()),
            writing_in: RwSignal::new(None),
            counted: RwSignal::new(Counted::Section),
        }
    }

    pub fn scene_has(self, scene: SceneId, words: usize) {
        let unchanged = self
            .words
            .with_untracked(|counted| counted.get(&scene) == Some(&words));

        if !unchanged {
            self.words
                .update(|counted| _ = counted.insert(scene, words));
        }
    }

    pub fn remember(self, counted: Vec<SceneWordCount>) {
        self.words.update(|words| {
            for count in counted {
                words.entry(count.scene).or_insert(count.words);
            }
        });
    }

    pub fn of_scene(self, scene: &SceneId) -> usize {
        self.words
            .with(|words| words.get(scene).copied().unwrap_or_default())
    }

    pub fn writing_in(self, section: SectionId) {
        if self
            .writing_in
            .with_untracked(|now| now.as_ref() != Some(&section))
        {
            self.writing_in.set(Some(section));
        }
    }

    pub fn toggle(self) {
        self.counted.update(|counted| {
            *counted = match counted {
                Counted::Section => Counted::Book,
                Counted::Book => Counted::Section,
            }
        });
    }

    pub fn counted(self) -> Counted {
        self.counted.get()
    }

    pub fn section(self) -> Option<SectionId> {
        self.writing_in
            .get()
            .filter(|section| self.structure.title_of(section).is_some())
            .or_else(|| self.structure.first_section())
    }

    pub fn words_in(self, scenes: &[SceneId]) -> usize {
        self.words
            .with(|counted| scenes.iter().filter_map(|scene| counted.get(scene)).sum())
    }

    pub fn words(self) -> usize {
        let scenes = match self.counted() {
            Counted::Book => self.structure.every_scene(),
            Counted::Section => self
                .section()
                .map(|section| self.structure.scenes_within(&section))
                .unwrap_or_default(),
        };

        self.words
            .with(|counted| scenes.iter().filter_map(|scene| counted.get(scene)).sum())
    }
}
