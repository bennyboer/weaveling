use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::scenes::model::{SceneId, SceneSummary};
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;
use crate::writing::saving::Saving;
use crate::writing::structure::Structure;

#[derive(Clone, Copy)]
pub struct LooseScenes {
    listed: RwSignal<Vec<SceneId>>,
    structure: Structure,
    problems: Problems,
    saving: Saving,
}

impl LooseScenes {
    pub fn of(structure: Structure, problems: Problems, saving: Saving) -> Self {
        Self {
            listed: RwSignal::new(Vec::new()),
            structure,
            problems,
            saving,
        }
    }

    pub fn delete(self, scene: SceneId) {
        let before = self.listed.get_untracked();
        self.listed
            .update(|listed| listed.retain(|held| held != &scene));
        self.saving.write_begun();

        spawn_local(async move {
            if let Err(failure) = scenes::delete(&scene).await {
                self.listed.set(before);
                self.problems.report(failure);
            }
            self.saving.write_done();
        });
    }

    pub fn remember(self, listed: &[SceneSummary]) {
        self.listed
            .set(listed.iter().map(|scene| scene.id.clone()).collect());
    }

    pub fn in_order(self) -> Vec<SceneId> {
        let held: HashSet<SceneId> = self.structure.every_scene().into_iter().collect();

        self.listed.with(|listed| {
            listed
                .iter()
                .filter(|scene| !held.contains(scene))
                .cloned()
                .collect()
        })
    }

    pub fn in_order_untracked(self) -> Vec<SceneId> {
        untrack(|| self.in_order())
    }
}
