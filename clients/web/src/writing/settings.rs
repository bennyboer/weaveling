use leptos::prelude::*;
use web_sys::Storage;

const SPELLING: &str = "weaveling.writing.spellcheck";
const OFF: &str = "off";

#[derive(Clone, Copy)]
pub struct Settings {
    spelling: RwSignal<bool>,
    open: RwSignal<bool>,
}

impl Settings {
    pub fn as_last_left() -> Self {
        let switched_off = shelf()
            .and_then(|shelf| shelf.get_item(SPELLING).ok().flatten())
            .is_some_and(|kept| kept == OFF);

        Self {
            spelling: RwSignal::new(!switched_off),
            open: RwSignal::new(false),
        }
    }

    pub fn checks_spelling(self) -> bool {
        self.spelling.get()
    }

    pub fn check_spelling(self, wanted: bool) {
        self.spelling.set(wanted);

        if let Some(shelf) = shelf() {
            _ = match wanted {
                true => shelf.remove_item(SPELLING),
                false => shelf.set_item(SPELLING, OFF),
            };
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

fn shelf() -> Option<Storage> {
    window().local_storage().ok().flatten()
}
