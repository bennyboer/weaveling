use leptos::prelude::*;

use crate::kept::{forget, keep, recall};

const SPELLING: &str = "weaveling.writing.spellcheck";
const JUSTIFIED: &str = "weaveling.writing.justified";
const OFF: &str = "off";
const ON: &str = "on";

#[derive(Clone, Copy)]
pub struct Settings {
    spelling: RwSignal<bool>,
    justified: RwSignal<bool>,
    open: RwSignal<bool>,
}

impl Settings {
    pub fn as_last_left() -> Self {
        Self {
            spelling: RwSignal::new(recall(SPELLING).as_deref() != Some(OFF)),
            justified: RwSignal::new(recall(JUSTIFIED).as_deref() == Some(ON)),
            open: RwSignal::new(false),
        }
    }

    pub fn checks_spelling(self) -> bool {
        self.spelling.get()
    }

    pub fn check_spelling(self, wanted: bool) {
        self.spelling.set(wanted);
        match wanted {
            true => forget(SPELLING),
            false => keep(SPELLING, OFF),
        }
    }

    pub fn justifies(self) -> bool {
        self.justified.get()
    }

    pub fn justify(self, wanted: bool) {
        self.justified.set(wanted);
        match wanted {
            true => keep(JUSTIFIED, ON),
            false => forget(JUSTIFIED),
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
