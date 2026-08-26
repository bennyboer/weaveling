use leptos::prelude::window;
use wasm_bindgen::JsValue;
use web_sys::UrlSearchParams;

pub fn query(name: &str) -> Option<String> {
    let search = window().location().search().ok()?;
    let params = UrlSearchParams::new_with_str(&search).ok()?;

    params.get(name).filter(|value| !value.is_empty())
}

pub fn remember(name: &str, value: &str) {
    rewrite(|params| params.set(name, value));
}

pub fn forget(name: &str) {
    rewrite(|params| params.delete(name));
}

fn rewrite(change: impl FnOnce(&UrlSearchParams)) {
    let location = window().location();
    let (Ok(path), Ok(search)) = (location.pathname(), location.search()) else {
        return;
    };
    let Ok(params) = UrlSearchParams::new_with_str(&search) else {
        return;
    };

    change(&params);

    let query = params.to_string().as_string().unwrap_or_default();
    let address = if query.is_empty() {
        path
    } else {
        format!("{path}?{query}")
    };

    if let Ok(history) = window().history() {
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&address));
    }
}
