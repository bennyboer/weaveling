use leptos::prelude::*;

use crate::drawer::Drawer;
use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inspected {
    Scene(SceneId),
    Section(SectionId),
}

#[derive(Clone, Copy)]
pub struct Inspector {
    inspected: RwSignal<Option<Inspected>>,
    drawer: Drawer,
}

impl Inspector {
    pub fn as_last_left() -> Self {
        Self {
            inspected: RwSignal::new(None),
            drawer: Drawer::as_last_left("weaveling.inspector", false),
        }
    }

    pub fn inspected(self) -> Option<Inspected> {
        self.inspected.get()
    }

    pub fn inspected_untracked(self) -> Option<Inspected> {
        self.inspected.get_untracked()
    }

    pub fn scene(self) -> Option<SceneId> {
        self.inspected.with(scene_in)
    }

    pub fn scene_untracked(self) -> Option<SceneId> {
        self.inspected.with_untracked(scene_in)
    }

    pub fn follow(self, inspected: Inspected) {
        if self
            .inspected
            .with_untracked(|held| held.as_ref() != Some(&inspected))
        {
            self.inspected.set(Some(inspected));
        }
    }

    pub fn drawer(self) -> Drawer {
        self.drawer
    }
}

fn scene_in(inspected: &Option<Inspected>) -> Option<SceneId> {
    match inspected {
        Some(Inspected::Scene(scene)) => Some(scene.clone()),
        _ => None,
    }
}
