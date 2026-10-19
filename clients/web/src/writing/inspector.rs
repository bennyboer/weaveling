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
    followed: RwSignal<Option<Inspected>>,
    drawer: Drawer,
}

impl Inspector {
    pub fn as_last_left() -> Self {
        Self {
            followed: RwSignal::new(None),
            drawer: Drawer::as_last_left("weaveling.inspector", false),
        }
    }

    pub fn followed(self) -> Option<Inspected> {
        self.followed.get()
    }

    pub fn scene(self) -> Option<SceneId> {
        self.followed.with(scene_in)
    }

    pub fn scene_untracked(self) -> Option<SceneId> {
        self.followed.with_untracked(scene_in)
    }

    pub fn follow(self, followed: Inspected) {
        if self
            .followed
            .with_untracked(|held| held.as_ref() != Some(&followed))
        {
            self.followed.set(Some(followed));
        }
    }

    pub fn drawer(self) -> Drawer {
        self.drawer
    }
}

fn scene_in(followed: &Option<Inspected>) -> Option<SceneId> {
    match followed {
        Some(Inspected::Scene(scene)) => Some(scene.clone()),
        _ => None,
    }
}
