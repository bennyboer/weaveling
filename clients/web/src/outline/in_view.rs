use std::collections::HashMap;

use leptos::prelude::*;
use web_sys::Element;

use crate::nearness::{KEPT, NEAR, Watching, entries_placed_by, observing};

pub const LEAVES_PLACE: &str = "data-leaves";

#[derive(Clone, Copy)]
pub struct InView {
    owner: StoredValue<Option<Owner>, LocalStorage>,
    near: StoredValue<HashMap<String, RwSignal<bool>>>,
    holding: Signal<bool>,
    watching: StoredValue<Vec<Watching>, LocalStorage>,
}

impl InView {
    pub fn around_the_window(holding: Signal<bool>) -> Self {
        let in_view = Self {
            owner: StoredValue::new_local(Owner::current()),
            near: StoredValue::new(HashMap::new()),
            holding,
            watching: StoredValue::new_local(Vec::new()),
        };

        let coming_near = observing(NEAR, move |entries| {
            for (leaves, entry) in entries_placed_by(LEAVES_PLACE, entries) {
                if entry.is_intersecting() {
                    in_view.set(&leaves, true);
                }
            }
        });
        let going_far = observing(KEPT, move |entries| {
            for (leaves, entry) in entries_placed_by(LEAVES_PLACE, entries) {
                if !entry.is_intersecting() && !in_view.holds(&entry.target()) {
                    in_view.set(&leaves, false);
                }
            }
        });
        in_view
            .watching
            .set_value(coming_near.into_iter().chain(going_far).collect());

        in_view
    }

    fn signal_of(self, leaves: &str) -> RwSignal<bool> {
        if let Some(near) = self.near.with_value(|near| near.get(leaves).copied()) {
            return near;
        }

        let made = self.owner.with_value(|owner| match owner {
            Some(owner) => owner.with(|| RwSignal::new(false)),
            None => RwSignal::new(false),
        });
        self.near
            .update_value(|near| _ = near.insert(leaves.to_owned(), made));

        made
    }

    fn set(self, leaves: &str, near: bool) {
        let signal = self.signal_of(leaves);

        if signal.try_get_untracked().is_some_and(|was| was != near) {
            signal.set(near);
        }
    }

    fn holds(self, place: &Element) -> bool {
        let holds_the_focus = document()
            .active_element()
            .is_some_and(|focused| place.contains(Some(&focused)));

        holds_the_focus || self.holding.try_get_untracked().unwrap_or(false)
    }

    pub fn is_near(self, leaves: &str) -> bool {
        self.signal_of(leaves).get()
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

    pub fn close(self) {
        _ = self.watching.try_with_value(|watching| {
            for (observer, _) in watching {
                observer.disconnect();
            }
        });
    }
}
