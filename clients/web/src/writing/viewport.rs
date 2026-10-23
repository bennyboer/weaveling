use std::collections::{HashMap, HashSet};

use js_sys::Array;
use leptos::html::Div;
use leptos::prelude::*;
use web_sys::{Element, IntersectionObserverEntry};

use crate::nearness;
use crate::nearness::Nearness;
use crate::scenes::model::SceneId;
use crate::writing::saving::Saving;

pub const SCENE_PLACE: &str = "data-place-of";

#[derive(Clone, Copy)]
pub struct Viewport {
    nearness: Nearness<SceneId>,
    heights: StoredValue<HashMap<SceneId, f64>>,
    leaving: StoredValue<HashSet<SceneId>>,
    landing: StoredValue<HashSet<SceneId>>,
    saving: Saving,
}

fn entries_of(entries: Array) -> Vec<(SceneId, IntersectionObserverEntry)> {
    nearness::entries_placed_by(SCENE_PLACE, entries)
        .into_iter()
        .map(|(scene, entry)| (SceneId::from(scene), entry))
        .collect()
}

impl Viewport {
    pub fn around_the_window(saving: Saving) -> Self {
        let viewport = Self {
            nearness: Nearness::around_the_window(),
            heights: StoredValue::new(HashMap::new()),
            leaving: StoredValue::new(HashSet::new()),
            landing: StoredValue::new(HashSet::new()),
            saving,
        };

        viewport.nearness.observing(
            move |entries| {
                for (scene, entry) in entries_of(entries) {
                    if entry.is_intersecting() {
                        _ = viewport
                            .landing
                            .try_update_value(|landing| landing.remove(&scene));
                        viewport.bring_near(scene);
                    }
                }
            },
            move |entries| {
                for (scene, entry) in entries_of(entries) {
                    if !entry.is_intersecting() {
                        let height = entry.bounding_client_rect().height();
                        viewport.went_far(scene, &entry.target(), height);
                    }
                }
            },
        );

        viewport
    }

    pub fn bring_near(self, scene: SceneId) {
        _ = self
            .leaving
            .try_update_value(|leaving| leaving.remove(&scene));
        self.nearness.set(&scene, true);
    }

    pub fn hold_for_landing(self, scene: SceneId) {
        _ = self
            .landing
            .try_update_value(|landing| landing.insert(scene.clone()));
        self.bring_near(scene);
    }

    fn went_far(self, scene: SceneId, place: &Element, height: f64) {
        let holds_the_cursor = document()
            .active_element()
            .is_some_and(|focused| place.contains(Some(&focused)));
        let being_landed_in = self
            .landing
            .try_with_value(|landing| landing.contains(&scene))
            .unwrap_or(false);

        if holds_the_cursor || being_landed_in {
            return;
        }
        _ = self
            .heights
            .try_update_value(|heights| heights.insert(scene.clone(), height));

        if self.saving.is_scene_saving(&scene) {
            _ = self
                .leaving
                .try_update_value(|leaving| leaving.insert(scene));
        } else {
            self.let_go(&scene);
        }
    }

    pub fn saved(self, scene: &SceneId) {
        let was_leaving = self
            .leaving
            .try_update_value(|leaving| leaving.remove(scene))
            .unwrap_or(false);

        if was_leaving {
            self.let_go(scene);
        }
    }

    fn let_go(self, scene: &SceneId) {
        self.nearness.set(scene, false);
    }

    pub fn kept_height(self, scene: &SceneId) -> Option<f64> {
        self.heights
            .try_with_value(|heights| heights.get(scene).copied())
            .flatten()
    }

    pub fn watched(self, scene: SceneId, place: NodeRef<Div>) -> Memo<bool> {
        self.nearness.watched(scene, place)
    }

    pub fn close(self) {
        self.nearness.close();
    }
}
