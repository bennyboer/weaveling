use leptos::prelude::*;
use web_sys::Storage;

use crate::scenes::model::SceneId;

const KEPT_OPEN: &str = "weaveling.scene-tray";
const OPEN: &str = "open";

#[derive(Clone, Copy)]
pub struct SceneTray {
    scene: RwSignal<Option<SceneId>>,
    open: RwSignal<bool>,
}

impl SceneTray {
    pub fn as_last_left() -> Self {
        let open = shelf()
            .and_then(|shelf| shelf.get_item(KEPT_OPEN).ok().flatten())
            .is_some_and(|kept| kept == OPEN);

        Self {
            scene: RwSignal::new(None),
            open: RwSignal::new(open),
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

    pub fn is_open(self) -> bool {
        self.open.get()
    }

    pub fn toggle(self) {
        self.keep_open(!self.open.get_untracked());
    }

    pub fn close(self) {
        self.keep_open(false);
    }

    fn keep_open(self, open: bool) {
        self.open.set(open);

        if let Some(shelf) = shelf() {
            _ = match open {
                true => shelf.set_item(KEPT_OPEN, OPEN),
                false => shelf.remove_item(KEPT_OPEN),
            };
        }
    }
}

fn shelf() -> Option<Storage> {
    window().local_storage().ok().flatten()
}
