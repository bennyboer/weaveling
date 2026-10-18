use std::collections::HashMap;
use std::hash::Hash;

use js_sys::Array;
use leptos::html::ElementType;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};

const NEAR: &str = "1500px 0px";
const KEPT: &str = "3000px 0px";

type Watching = (
    IntersectionObserver,
    Closure<dyn Fn(Array, IntersectionObserver)>,
);

pub fn entries_placed_by(
    place: &'static str,
    entries: Array,
) -> Vec<(String, IntersectionObserverEntry)> {
    entries
        .iter()
        .filter_map(|entry| entry.dyn_into::<IntersectionObserverEntry>().ok())
        .filter_map(|entry| {
            entry
                .target()
                .get_attribute(place)
                .map(|placed| (placed, entry))
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

pub struct Nearness<K: Send + Sync + 'static> {
    owner: StoredValue<Option<Owner>, LocalStorage>,
    near: StoredValue<HashMap<K, RwSignal<bool>>>,
    watching: StoredValue<Vec<Watching>, LocalStorage>,
}

impl<K: Send + Sync + 'static> Clone for Nearness<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Send + Sync + 'static> Copy for Nearness<K> {}

impl<K: Eq + Hash + Clone + Send + Sync + 'static> Nearness<K> {
    pub fn around_the_window() -> Self {
        Self {
            owner: StoredValue::new_local(Owner::current()),
            near: StoredValue::new(HashMap::new()),
            watching: StoredValue::new_local(Vec::new()),
        }
    }

    pub fn observing(
        self,
        coming_near: impl Fn(Array) + 'static,
        going_far: impl Fn(Array) + 'static,
    ) {
        let observers = observing(NEAR, coming_near)
            .into_iter()
            .chain(observing(KEPT, going_far));
        self.watching.set_value(observers.collect());
    }

    fn signal_of(self, key: &K) -> RwSignal<bool> {
        if let Some(near) = self.near.with_value(|near| near.get(key).copied()) {
            return near;
        }

        let made = self.owner.with_value(|owner| match owner {
            Some(owner) => owner.with(|| RwSignal::new(false)),
            None => RwSignal::new(false),
        });
        self.near
            .update_value(|near| _ = near.insert(key.clone(), made));

        made
    }

    pub fn set(self, key: &K, near: bool) {
        let signal = self.signal_of(key);

        if signal.try_get_untracked().is_some_and(|was| was != near) {
            signal.set(near);
        }
    }

    pub fn watched<E>(self, key: K, place: NodeRef<E>) -> Memo<bool>
    where
        E: ElementType,
        E::Output: JsCast + Clone + 'static,
    {
        Effect::new(move || {
            if let Some(watched) = place.get() {
                self.each_observer(|observer| observer.observe(watched.unchecked_ref::<Element>()));
            }
        });
        on_cleanup(move || {
            if let Some(watched) = place.get_untracked() {
                self.each_observer(|observer| {
                    observer.unobserve(watched.unchecked_ref::<Element>())
                });
            }
        });

        Memo::new(move |_| self.signal_of(&key).get())
    }

    pub fn close(self) {
        self.each_observer(IntersectionObserver::disconnect);
    }

    fn each_observer(self, act: impl Fn(&IntersectionObserver)) {
        _ = self
            .watching
            .try_with_value(|watching| watching.iter().for_each(|(observer, _)| act(observer)));
    }
}
