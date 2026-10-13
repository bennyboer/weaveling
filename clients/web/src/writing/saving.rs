use std::collections::HashSet;

use leptos::prelude::*;

use crate::scenes::model::SceneId;

#[derive(Clone, Copy)]
pub struct Saving {
    scenes: RwSignal<HashSet<SceneId>>,
    writes: RwSignal<usize>,
}

impl Saving {
    pub fn nothing() -> Self {
        Self {
            scenes: RwSignal::new(HashSet::new()),
            writes: RwSignal::new(0),
        }
    }

    pub fn scene_is_saving(self, scene: SceneId, saving: bool) {
        let known = self.scenes.with_untracked(|scenes| scenes.contains(&scene));

        match (saving, known) {
            (true, false) => self.scenes.update(|scenes| _ = scenes.insert(scene)),
            (false, true) => self.scenes.update(|scenes| _ = scenes.remove(&scene)),
            _ => {}
        }
    }

    pub fn let_go_of(self, scene: &SceneId) {
        self.scene_is_saving(scene.clone(), false);
    }

    pub fn write_begun(self) {
        self.writes.update(|writes| *writes += 1);
    }

    pub fn write_done(self) {
        _ = self
            .writes
            .try_update(|writes| *writes = writes.saturating_sub(1));
    }

    pub fn is_saving(self) -> bool {
        self.writes.get() > 0 || self.scenes.with(|scenes| !scenes.is_empty())
    }
}
