use std::collections::{HashMap, HashSet};

use leptos::prelude::*;

use crate::scenes::model::SceneId;
use crate::statistics::model::SceneWordCount;
use crate::writing::structure::Structure;
use crate::writing::writing_in::WritingIn;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counted {
    Section,
    Book,
}

#[derive(Clone, Copy)]
pub struct WordCount {
    structure: Structure,
    writing_in: WritingIn,
    words: RwSignal<HashMap<SceneId, usize>>,
    counting_here: StoredValue<HashSet<SceneId>>,
    counted: RwSignal<Counted>,
}

impl WordCount {
    pub fn of(structure: Structure, writing_in: WritingIn) -> Self {
        Self {
            structure,
            writing_in,
            words: RwSignal::new(HashMap::new()),
            counting_here: StoredValue::new(HashSet::new()),
            counted: RwSignal::new(Counted::Section),
        }
    }

    pub fn scene_has(self, scene: SceneId, words: usize) {
        self.counting_here
            .update_value(|here| _ = here.insert(scene.clone()));
        let unchanged = self
            .words
            .with_untracked(|counted| counted.get(&scene) == Some(&words));

        if !unchanged {
            self.words
                .update(|counted| _ = counted.insert(scene, words));
        }
    }

    pub fn let_go_of(self, scene: &SceneId) {
        _ = self
            .counting_here
            .try_update_value(|here| _ = here.remove(scene));
    }

    pub fn told(self, counted: Vec<SceneWordCount>) {
        let elsewhere = self.counting_here.with_value(|here| {
            counted
                .into_iter()
                .filter(|count| !here.contains(&count.scene))
                .collect::<Vec<_>>()
        });

        self.words.update(|words| {
            for count in elsewhere {
                words.insert(count.scene, count.words);
            }
        });
    }

    pub fn of_scene(self, scene: &SceneId) -> usize {
        self.words
            .with(|words| words.get(scene).copied().unwrap_or_default())
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

    pub fn words_in(self, scenes: &[SceneId]) -> usize {
        self.words
            .with(|counted| scenes.iter().filter_map(|scene| counted.get(scene)).sum())
    }

    pub fn words(self) -> usize {
        let scenes = match self.counted() {
            Counted::Book => self.structure.every_scene(),
            Counted::Section => self
                .writing_in
                .section()
                .map(|section| self.structure.scenes_within(&section))
                .unwrap_or_default(),
        };

        self.words
            .with(|counted| scenes.iter().filter_map(|scene| counted.get(scene)).sum())
    }
}
