use leptos::prelude::*;
use web_sys::Storage;

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
            spelling: RwSignal::new(kept(SPELLING).as_deref() != Some(OFF)),
            justified: RwSignal::new(kept(JUSTIFIED).as_deref() == Some(ON)),
            open: RwSignal::new(false),
        }
    }

    pub fn checks_spelling(self) -> bool {
        self.spelling.get()
    }

    pub fn check_spelling(self, wanted: bool) {
        self.spelling.set(wanted);
        keep(SPELLING, (!wanted).then_some(OFF));
    }

    pub fn justifies(self) -> bool {
        self.justified.get()
    }

    pub fn justify(self, wanted: bool) {
        self.justified.set(wanted);
        keep(JUSTIFIED, wanted.then_some(ON));
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

fn kept(key: &str) -> Option<String> {
    shelf().and_then(|shelf| shelf.get_item(key).ok().flatten())
}

fn keep(key: &str, value: Option<&str>) {
    if let Some(shelf) = shelf() {
        _ = match value {
            Some(value) => shelf.set_item(key, value),
            None => shelf.remove_item(key),
        };
    }
}

fn shelf() -> Option<Storage> {
    window().local_storage().ok().flatten()
}
