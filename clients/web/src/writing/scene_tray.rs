use leptos::prelude::*;

use crate::drawer::Drawer;
use crate::scenes::model::SceneId;

#[derive(Clone, Copy)]
pub struct SceneTray {
    scene: RwSignal<Option<SceneId>>,
    drawer: Drawer,
}

impl SceneTray {
    pub fn as_last_left() -> Self {
        Self {
            scene: RwSignal::new(None),
            drawer: Drawer::as_last_left("weaveling.scene-tray", false),
        }
    }

    pub fn scene(self) -> Option<SceneId> {
        self.scene.get()
    }

    pub fn scene_untracked(self) -> Option<SceneId> {
        self.scene.get_untracked()
    }

    pub fn follow(self, scene: SceneId) {
        if self
            .scene
            .with_untracked(|held| held.as_ref() != Some(&scene))
        {
            self.scene.set(Some(scene));
        }
    }

    pub fn drawer(self) -> Drawer {
        self.drawer
    }
}
