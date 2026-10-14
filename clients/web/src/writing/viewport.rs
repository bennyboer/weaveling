use std::collections::HashSet;

use js_sys::Array;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};

use crate::scenes::model::SceneId;

const NEAR: &str = "1500px 0px";

pub const SCENE_PLACE: &str = "data-place-of";

type Watching = (
    IntersectionObserver,
    Closure<dyn Fn(Array, IntersectionObserver)>,
);

#[derive(Clone, Copy)]
pub struct Viewport {
    near: RwSignal<HashSet<SceneId>>,
    watching: StoredValue<Option<Watching>, LocalStorage>,
}

impl Viewport {
    pub fn around_the_window() -> Self {
        let viewport = Self {
            near: RwSignal::new(HashSet::new()),
            watching: StoredValue::new_local(None),
        };

        let came_near = Closure::<dyn Fn(Array, IntersectionObserver)>::new(
            move |entries: Array, _: IntersectionObserver| {
                let near: Vec<SceneId> = entries
                    .iter()
                    .filter_map(|entry| entry.dyn_into::<IntersectionObserverEntry>().ok())
                    .filter(IntersectionObserverEntry::is_intersecting)
                    .filter_map(|entry| entry.target().get_attribute(SCENE_PLACE))
                    .map(SceneId::from)
                    .collect();

                for scene in near {
                    viewport.bring_near(scene);
                }
            },
        );
        let reaching = IntersectionObserverInit::new();
        reaching.set_root_margin(NEAR);

        if let Ok(observer) =
            IntersectionObserver::new_with_options(came_near.as_ref().unchecked_ref(), &reaching)
        {
            viewport.watching.set_value(Some((observer, came_near)));
        }

        viewport
    }

    pub fn watch(self, place: &Element) {
        self.watching.with_value(|watching| {
            if let Some((observer, _)) = watching {
                observer.observe(place);
            }
        });
    }

    pub fn stop_watching(self, place: &Element) {
        _ = self.watching.try_with_value(|watching| {
            if let Some((observer, _)) = watching {
                observer.unobserve(place);
            }
        });
    }

    pub fn bring_near(self, scene: SceneId) {
        let known = self
            .near
            .try_with_untracked(|near| near.contains(&scene))
            .unwrap_or(true);

        if !known {
            self.near.update(|near| _ = near.insert(scene));
        }
    }

    pub fn is_near(self, scene: &SceneId) -> bool {
        self.near.with(|near| near.contains(scene))
    }

    pub fn close(self) {
        _ = self.watching.try_with_value(|watching| {
            if let Some((observer, _)) = watching {
                observer.disconnect();
            }
        });
    }
}
