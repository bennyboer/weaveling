use web_sys::Storage;

pub fn recall(key: &str) -> Option<String> {
    shelf()?.get_item(key).ok().flatten()
}

pub fn keep(key: &str, value: &str) {
    if let Some(shelf) = shelf() {
        _ = shelf.set_item(key, value);
    }
}

pub fn forget(key: &str) {
    if let Some(shelf) = shelf() {
        _ = shelf.remove_item(key);
    }
}

fn shelf() -> Option<Storage> {
    leptos::prelude::window().local_storage().ok().flatten()
}
