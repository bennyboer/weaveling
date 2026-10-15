use js_sys::Array;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};

pub const NEAR: &str = "1500px 0px";
pub const KEPT: &str = "3000px 0px";

pub type Watching = (
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

pub fn observing(margin: &str, seen: impl Fn(Array) + 'static) -> Option<Watching> {
    let callback = Closure::<dyn Fn(Array, IntersectionObserver)>::new(
        move |entries: Array, _: IntersectionObserver| seen(entries),
    );
    let reaching = IntersectionObserverInit::new();
    reaching.set_root_margin(margin);

    IntersectionObserver::new_with_options(callback.as_ref().unchecked_ref(), &reaching)
        .ok()
        .map(|observer| (observer, callback))
}
