use std::collections::HashMap;

use leptos::prelude::*;

use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;

#[derive(Clone, Copy)]
pub struct Editors {
    held: StoredValue<HashMap<SceneId, ProseEditor>, LocalStorage>,
}

impl Editors {
    pub fn none() -> Self {
        Self {
            held: StoredValue::new_local(HashMap::new()),
        }
    }

    pub fn arrived(self, scene: SceneId, editor: ProseEditor) {
        self.held
            .update_value(|held| _ = held.insert(scene, editor));
    }

    pub fn gone(self, scene: &SceneId) -> Option<ProseEditor> {
        self.held
            .try_update_value(|held| held.remove(scene))
            .flatten()
    }

    pub fn told<T>(self, scene: &SceneId, told: impl FnOnce(&ProseEditor) -> T) -> Option<T> {
        self.held
            .try_with_value(|held| held.get(scene).map(told))
            .flatten()
    }

    pub fn text_in(self, scene: &SceneId) -> Option<String> {
        self.told(scene, ProseEditor::quoted_text)
    }

    pub fn back_into(self, scene: &SceneId) -> bool {
        self.told(scene, ProseEditor::focus).is_some()
    }
}
