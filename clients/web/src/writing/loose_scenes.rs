use std::collections::HashSet;

use leptos::prelude::*;

use crate::scenes::model::{SceneId, SceneSummary};
use crate::writing::structure::Structure;

#[derive(Clone, Copy)]
pub struct LooseScenes {
    listed: RwSignal<Vec<SceneId>>,
    structure: Structure,
}

impl LooseScenes {
    pub fn of(structure: Structure) -> Self {
        Self {
            listed: RwSignal::new(Vec::new()),
            structure,
        }
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
