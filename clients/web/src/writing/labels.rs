use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::projects::model::ProjectId;
use crate::scenes::model::{SceneId, opening_of};
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;

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
}

impl Labels {
    pub fn of(project: &ProjectId, problems: Problems) -> Self {
        let labels = Self {
            titles: RwSignal::new(HashMap::new()),
            openings: RwSignal::new(HashMap::new()),
            problems,
        };
        let project = project.clone();

        spawn_local(async move {
            match scenes::in_project(&project).await {
                Ok(listed) => labels.titles.set(
                    listed
                        .into_iter()
                        .map(|scene| (scene.id, scene.title))
                        .collect(),
                ),
                Err(failure) => labels.problems.report(failure),
            }
        });

        labels
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

        spawn_local(async move {
            if let Err(failure) = scenes::retitle(&scene, &title).await {
                self.titles
                    .update(|titles| _ = titles.insert(scene, before));
                self.problems.report(failure);
            }
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
