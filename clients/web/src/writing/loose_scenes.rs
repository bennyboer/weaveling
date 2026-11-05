use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task;

use crate::scenes::model::{SceneId, SceneSummary};
use crate::writing::scene_copies::SceneCopies;
use crate::writing::structure::Structure;

#[derive(Clone, Copy)]
pub struct LooseScenes {
    listed: RwSignal<Option<Vec<SceneId>>>,
    structure: Structure,
    scenes: SceneCopies,
}

impl LooseScenes {
    pub fn of(structure: Structure, scenes: SceneCopies) -> Self {
        Self {
            listed: RwSignal::new(None),
            structure,
            scenes,
        }
    }

    pub fn delete(self, scene: SceneId) {
        let before = self.listed.get_untracked();
        self.listed.update(|listed| {
            if let Some(listed) = listed {
                listed.retain(|held| held != &scene);
            }
        });
        let deleting = self.scenes.deleted(scene);

        task::spawn_local(async move {
            if !deleting.await {
                _ = self.listed.try_set(before);
            }
        });
    }

    pub fn remember(self, listed: &[SceneSummary]) {
        self.listed
            .set(Some(listed.iter().map(|scene| scene.id.clone()).collect()));
    }

    pub fn include(self, scenes: Vec<SceneId>) {
        self.listed.update(|listed| {
            if let Some(listed) = listed {
                for scene in scenes {
                    if !listed.contains(&scene) {
                        let at = listed.partition_point(|held| held.as_str() < scene.as_str());
                        listed.insert(at, scene);
                    }
                }
            }
        });
    }

    pub fn is_known(self) -> bool {
        self.listed.with(Option::is_some)
    }

    pub fn in_order(self) -> Vec<SceneId> {
        let held: HashSet<SceneId> = self.structure.every_scene().into_iter().collect();

        self.listed.with(|listed| {
            listed
                .iter()
                .flatten()
                .filter(|scene| !held.contains(scene))
                .cloned()
                .collect()
        })
    }

    pub fn in_order_untracked(self) -> Vec<SceneId> {
        untrack(|| self.in_order())
    }
}
