use leptos::ev;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

pub fn typed(event: &ev::Event) -> Option<HtmlInputElement> {
    event
        .target()
        .and_then(|it| it.dyn_into::<HtmlInputElement>().ok())
}
