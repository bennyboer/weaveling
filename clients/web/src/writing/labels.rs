use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task;

use crate::scenes::model::{SceneId, SceneSummary};
use crate::writing::scene_copies::SceneCopies;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub shown: String,
    pub titled: bool,
}

#[derive(Clone, Copy)]
pub struct Labels {
    titles: RwSignal<HashMap<SceneId, String>>,
    openings: RwSignal<HashMap<SceneId, String>>,
    opened_here: StoredValue<HashSet<SceneId>>,
    renaming: StoredValue<HashSet<SceneId>>,
    scenes: SceneCopies,
}

impl Labels {
    pub fn of(scenes: SceneCopies) -> Self {
        Self {
            titles: RwSignal::new(HashMap::new()),
            openings: RwSignal::new(HashMap::new()),
            opened_here: StoredValue::new(HashSet::new()),
            renaming: StoredValue::new(HashSet::new()),
            scenes,
        }
    }

    pub fn remember(self, listed: &[SceneSummary]) {
        self.titles.update(|titles| {
            let renaming = self.renaming.get_value();
            titles.retain(|scene, _| renaming.contains(scene));
            for scene in listed {
                if !renaming.contains(&scene.id) {
                    titles.insert(scene.id.clone(), scene.title.clone());
                }
            }
        });
        self.openings.update(|openings| {
            for scene in listed {
                openings
                    .entry(scene.id.clone())
                    .or_insert_with(|| scene.opening.clone());
            }
        });
    }

    pub fn told(self, scene: &SceneSummary) {
        if !self
            .renaming
            .with_value(|renaming| renaming.contains(&scene.id))
        {
            self.titles
                .update(|titles| _ = titles.insert(scene.id.clone(), scene.title.clone()));
        }
        if !self.opened_here.with_value(|here| here.contains(&scene.id)) {
            self.openings
                .update(|openings| _ = openings.insert(scene.id.clone(), scene.opening.clone()));
        }
    }

    pub fn let_go_of(self, scene: &SceneId) {
        _ = self
            .opened_here
            .try_update_value(|here| _ = here.remove(scene));
    }

    pub fn opening_is(self, scene: SceneId, text: String) {
        self.opened_here
            .update_value(|here| _ = here.insert(scene.clone()));
        let opening = words::opening_of(&text);
        let unchanged = self
            .openings
            .with_untracked(|openings| openings.get(&scene) == Some(&opening));

        if !unchanged {
            self.openings
                .update(|openings| _ = openings.insert(scene, opening));
        }
    }

    pub fn title_of(self, scene: &SceneId) -> String {
        self.titles
            .with_untracked(|titles| titles.get(scene).cloned().unwrap_or_default())
    }

    pub fn rename(self, scene: SceneId, title: String) {
        let before = self.title_of(&scene);
        if before == title {
            return;
        }
        self.titles
            .update(|titles| _ = titles.insert(scene.clone(), title.clone()));
        self.renaming
            .update_value(|renaming| _ = renaming.insert(scene.clone()));
        let renaming = self.scenes.retitled(scene.clone(), title);

        task::spawn_local(async move {
            if !renaming.await {
                _ = self
                    .titles
                    .try_update(|titles| _ = titles.insert(scene.clone(), before));
            }
            _ = self
                .renaming
                .try_update_value(|renaming| _ = renaming.remove(&scene));
        });
    }

    pub fn of_scene(self, scene: &SceneId) -> Label {
        let titled = self.titles.with(|titles| {
            titles
                .get(scene)
                .filter(|title| !title.trim().is_empty())
                .cloned()
        });

        match titled {
            Some(title) => Label {
                shown: title,
                titled: true,
            },
            None => Label {
                shown: self
                    .openings
                    .with(|openings| openings.get(scene).cloned().unwrap_or_default()),
                titled: false,
            },
        }
    }
}
