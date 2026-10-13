use std::collections::HashSet;

use leptos::prelude::*;

use crate::scenes::model::SceneId;

#[derive(Clone, Copy)]
pub struct Saving {
    scenes: RwSignal<HashSet<SceneId>>,
}

impl Saving {
    pub fn nothing() -> Self {
        Self {
            scenes: RwSignal::new(HashSet::new()),
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

    pub fn is_saving(self) -> bool {
        self.scenes.with(|scenes| !scenes.is_empty())
    }
}
