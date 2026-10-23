use leptos::ev;
use leptos::prelude::*;

use crate::kept;
use crate::kept::keep;

const AT_FIRST: f64 = 320.0;
pub const NARROWEST: f64 = 224.0;
pub const PAGE_LEFT_IN_SIGHT: f64 = 160.0;
const SHARE_OF_A_SMALL_WINDOW: f64 = 0.85;

#[derive(Clone, Copy)]
pub struct DrawerWidth {
    kept: StoredValue<String>,
    chosen: RwSignal<f64>,
    window: RwSignal<f64>,
    room: RwSignal<Option<f64>>,
}

impl DrawerWidth {
    pub fn as_last_left(kept: String) -> Self {
        let chosen = kept::recall(&kept)
            .and_then(|kept| kept.parse().ok())
            .filter(|width: &f64| width.is_finite())
            .unwrap_or(AT_FIRST);
        let window = RwSignal::new(window_width());

        let resizing = window_event_listener(ev::resize, move |_| window.set(window_width()));
        on_cleanup(move || resizing.remove());

        Self {
            kept: StoredValue::new(kept),
            chosen: RwSignal::new(chosen),
            window,
            room: RwSignal::new(None),
        }
    }

    pub fn shown(self) -> f64 {
        self.shown_in(self.room.get())
    }

    pub fn shown_in(self, room: Option<f64>) -> f64 {
        within(self.chosen.get(), self.window.get()).min(room.unwrap_or(f64::INFINITY))
    }

    pub fn widest(self) -> f64 {
        widest_for(self.window.get()).min(self.room.get().unwrap_or(f64::INFINITY))
    }

    pub fn window(self) -> f64 {
        self.window.get()
    }

    pub fn share(self, room: Option<f64>) {
        if self.room.get_untracked() != room {
            self.room.set(room);
        }
    }

    pub fn narrowest(self) -> f64 {
        NARROWEST.min(self.widest())
    }

    pub fn shown_untracked(self) -> f64 {
        untrack(|| self.shown())
    }

    pub fn widen_to(self, width: f64) {
        let room = self.room.get_untracked().unwrap_or(f64::INFINITY);

        self.chosen
            .set(within(width, self.window.get_untracked()).min(room));
    }

    pub fn keep(self) {
        let width = self.shown_untracked().round().to_string();
        self.kept.with_value(|kept| keep(kept, &width));
    }
}

fn widest_for(window: f64) -> f64 {
    (window - PAGE_LEFT_IN_SIGHT).max(window * SHARE_OF_A_SMALL_WINDOW)
}

fn within(width: f64, window: f64) -> f64 {
    width.max(NARROWEST).min(widest_for(window))
}

fn window_width() -> f64 {
    window()
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .unwrap_or(AT_FIRST * 4.0)
}
