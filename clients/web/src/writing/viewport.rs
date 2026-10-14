use std::collections::{HashMap, HashSet};

use js_sys::Array;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};

use crate::scenes::model::SceneId;
use crate::writing::saving::Saving;

const NEAR: &str = "1500px 0px";
const KEPT: &str = "3000px 0px";

pub const SCENE_PLACE: &str = "data-place-of";

type Watching = (
    IntersectionObserver,
    Closure<dyn Fn(Array, IntersectionObserver)>,
);

#[derive(Clone, Copy)]
pub struct Viewport {
    owner: StoredValue<Option<Owner>, LocalStorage>,
    near: StoredValue<HashMap<SceneId, RwSignal<bool>>>,
    heights: StoredValue<HashMap<SceneId, f64>>,
    leaving: StoredValue<HashSet<SceneId>>,
    landing: StoredValue<HashSet<SceneId>>,
    saving: Saving,
    watching: StoredValue<Vec<Watching>, LocalStorage>,
}

fn entries_of(entries: Array) -> Vec<(SceneId, IntersectionObserverEntry)> {
    entries
        .iter()
        .filter_map(|entry| entry.dyn_into::<IntersectionObserverEntry>().ok())
        .filter_map(|entry| {
            entry
                .target()
                .get_attribute(SCENE_PLACE)
                .map(|scene| (SceneId::from(scene), entry))
        })
        .collect()
}

fn observing(margin: &str, seen: impl Fn(Array) + 'static) -> Option<Watching> {
    let callback = Closure::<dyn Fn(Array, IntersectionObserver)>::new(
        move |entries: Array, _: IntersectionObserver| seen(entries),
    );
    let reaching = IntersectionObserverInit::new();
    reaching.set_root_margin(margin);

    IntersectionObserver::new_with_options(callback.as_ref().unchecked_ref(), &reaching)
        .ok()
        .map(|observer| (observer, callback))
}

impl Viewport {
    pub fn around_the_window(saving: Saving) -> Self {
        let viewport = Self {
            owner: StoredValue::new_local(Owner::current()),
            near: StoredValue::new(HashMap::new()),
            heights: StoredValue::new(HashMap::new()),
            leaving: StoredValue::new(HashSet::new()),
            landing: StoredValue::new(HashSet::new()),
            saving,
            watching: StoredValue::new_local(Vec::new()),
        };

        let coming_near = observing(NEAR, move |entries| {
            for (scene, entry) in entries_of(entries) {
                if entry.is_intersecting() {
                    _ = viewport
                        .landing
                        .try_update_value(|landing| landing.remove(&scene));
                    viewport.bring_near(scene);
                }
            }
        });
        let going_far = observing(KEPT, move |entries| {
            for (scene, entry) in entries_of(entries) {
                if !entry.is_intersecting() {
                    let height = entry.bounding_client_rect().height();
                    viewport.went_far(scene, &entry.target(), height);
                }
            }
        });
        viewport
            .watching
            .set_value(coming_near.into_iter().chain(going_far).collect());

        viewport
    }

    fn signal_of(self, scene: &SceneId) -> RwSignal<bool> {
        if let Some(near) = self.near.with_value(|near| near.get(scene).copied()) {
            return near;
        }

        let made = self.owner.with_value(|owner| match owner {
            Some(owner) => owner.with(|| RwSignal::new(false)),
            None => RwSignal::new(false),
        });
        self.near
            .update_value(|near| _ = near.insert(scene.clone(), made));

        made
    }

    pub fn watch(self, place: &Element) {
        self.watching.with_value(|watching| {
            for (observer, _) in watching {
                observer.observe(place);
            }
        });
    }

    pub fn stop_watching(self, place: &Element) {
        _ = self.watching.try_with_value(|watching| {
            for (observer, _) in watching {
                observer.unobserve(place);
            }
        });
    }

    pub fn is_near(self, scene: &SceneId) -> bool {
        self.signal_of(scene).get()
    }

    pub fn bring_near(self, scene: SceneId) {
        _ = self
            .leaving
            .try_update_value(|leaving| leaving.remove(&scene));
        let near = self.signal_of(&scene);

        if !near.try_get_untracked().unwrap_or(true) {
            near.set(true);
        }
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
        let near = self.signal_of(scene);

        if near.try_get_untracked().unwrap_or(false) {
            near.set(false);
        }
    }

    pub fn kept_height(self, scene: &SceneId) -> Option<f64> {
        self.heights
            .try_with_value(|heights| heights.get(scene).copied())
            .flatten()
    }

    pub fn close(self) {
        _ = self.watching.try_with_value(|watching| {
            for (observer, _) in watching {
                observer.disconnect();
            }
        });
    }
}
