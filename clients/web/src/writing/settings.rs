use leptos::prelude::*;

use crate::kept;

const SPELLING: &str = "weaveling.writing.spellcheck";
const JUSTIFIED: &str = "weaveling.writing.justified";
const TYPEWRITER: &str = "weaveling.writing.typewriter";
const OFF: &str = "off";
const ON: &str = "on";

#[derive(Clone, Copy)]
pub struct Settings {
    spelling: RwSignal<bool>,
    justified: RwSignal<bool>,
    typewriter: RwSignal<bool>,
    open: RwSignal<bool>,
}

impl Settings {
    pub fn as_last_left() -> Self {
        Self {
            spelling: RwSignal::new(kept::recall(SPELLING).as_deref() != Some(OFF)),
            justified: RwSignal::new(kept::recall(JUSTIFIED).as_deref() == Some(ON)),
            typewriter: RwSignal::new(kept::recall(TYPEWRITER).as_deref() == Some(ON)),
            open: RwSignal::new(false),
        }
    }

    pub fn checks_spelling(self) -> bool {
        self.spelling.get()
    }

    pub fn check_spelling(self, wanted: bool) {
        self.spelling.set(wanted);
        match wanted {
            true => kept::forget(SPELLING),
            false => kept::keep(SPELLING, OFF),
        }
    }

    pub fn justifies(self) -> bool {
        self.justified.get()
    }

    pub fn justify(self, wanted: bool) {
        self.justified.set(wanted);
        match wanted {
            true => kept::keep(JUSTIFIED, ON),
            false => kept::forget(JUSTIFIED),
        }
    }

    pub fn scrolls_like_a_typewriter(self) -> bool {
        self.typewriter.get()
    }

    pub fn scroll_like_a_typewriter(self, wanted: bool) {
        self.typewriter.set(wanted);
        match wanted {
            true => kept::keep(TYPEWRITER, ON),
            false => kept::forget(TYPEWRITER),
        }
    }

    pub fn is_open(self) -> bool {
        self.open.get()
    }

    pub fn open(self) {
        self.open.set(true);
    }

    pub fn close(self) {
        self.open.set(false);
    }
}
