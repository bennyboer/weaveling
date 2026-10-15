use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;

use words::opening_of;

use crate::scenes::model::{SceneId, SceneSummary};
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;
use crate::writing::saving::Saving;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub shown: String,
    pub titled: bool,
}

#[derive(Clone, Copy)]
pub struct Labels {
    titles: RwSignal<HashMap<SceneId, String>>,
    openings: RwSignal<HashMap<SceneId, String>>,
    problems: Problems,
    saving: Saving,
}

impl Labels {
    pub fn of(problems: Problems, saving: Saving) -> Self {
        Self {
            titles: RwSignal::new(HashMap::new()),
            openings: RwSignal::new(HashMap::new()),
            problems,
            saving,
        }
    }

    pub fn remember(self, listed: &[SceneSummary]) {
        self.titles.set(
            listed
                .iter()
                .map(|scene| (scene.id.clone(), scene.title.clone()))
                .collect(),
        );
        self.openings.update(|openings| {
            for scene in listed {
                openings
                    .entry(scene.id.clone())
                    .or_insert_with(|| scene.opening.clone());
            }
        });
    }

    pub fn opening_is(self, scene: SceneId, text: String) {
        let opening = opening_of(&text);
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
        self.saving.write_begun();

        spawn_local(async move {
            if let Err(failure) = scenes::retitle(&scene, &title).await {
                self.titles
                    .update(|titles| _ = titles.insert(scene, before));
                self.problems.report(failure);
            }
            self.saving.write_done();
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
