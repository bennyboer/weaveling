use leptos::html::Li;
use leptos::prelude::*;
use web_sys::Element;

use crate::nearness::{Nearness, entries_placed_by};

pub const LEAVES_PLACE: &str = "data-leaves";

#[derive(Clone, Copy)]
pub struct InView {
    nearness: Nearness<String>,
    holding: Signal<bool>,
}

impl InView {
    pub fn around_the_window(holding: Signal<bool>) -> Self {
        let in_view = Self {
            nearness: Nearness::around_the_window(),
            holding,
        };

        in_view.nearness.observing(
            move |entries| {
                for (leaves, entry) in entries_placed_by(LEAVES_PLACE, entries) {
                    if entry.is_intersecting() {
                        in_view.nearness.set(&leaves, true);
                    }
                }
            },
            move |entries| {
                for (leaves, entry) in entries_placed_by(LEAVES_PLACE, entries) {
                    if !entry.is_intersecting() && !in_view.holds(&entry.target()) {
                        in_view.nearness.set(&leaves, false);
                    }
                }
            },
        );

        in_view
    }

    fn holds(self, place: &Element) -> bool {
        let holds_the_focus = document()
            .active_element()
            .is_some_and(|focused| place.contains(Some(&focused)));

        holds_the_focus || self.holding.try_get_untracked().unwrap_or(false)
    }

    pub fn watched(self, leaves: String, place: NodeRef<Li>) -> Memo<bool> {
        self.nearness.watched(leaves, place)
    }

    pub fn close(self) {
        self.nearness.close();
    }
}
