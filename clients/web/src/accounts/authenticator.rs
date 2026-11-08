use serde_json::Value;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(module = "/js/editor.bundle.js")]
extern "C" {
    #[wasm_bindgen(js_name = keysSupported)]
    fn keys_supported() -> bool;

    #[wasm_bindgen(js_name = makeKey, catch)]
    async fn make_key(asked: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = touchKey, catch)]
    async fn touch_key(asked: &str) -> Result<JsValue, JsValue>;
}

/// Why the browser made or showed no key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoKey {
    Cancelled,
    HeldAlready,
    Failed,
}

/// Whether this browser can make keys and ask for them.
pub fn supported() -> bool {
    keys_supported()
}

/// The browser's dialog making a key, as the server asked for it.
pub async fn made(asked: &Value) -> Result<Value, NoKey> {
    answered(make_key(&asked.to_string()).await)
}

/// The browser's dialog asking for a touch of a key, as the server asked for it.
pub async fn touched(asked: &Value) -> Result<Value, NoKey> {
    answered(touch_key(&asked.to_string()).await)
}

fn answered(given: Result<JsValue, JsValue>) -> Result<Value, NoKey> {
    let written = given.map_err(|failed| {
        match js_sys::Reflect::get(&failed, &JsValue::from_str("name"))
            .ok()
            .and_then(|name| name.as_string())
            .as_deref()
        {
            Some("NotAllowedError" | "AbortError") => NoKey::Cancelled,
            Some("InvalidStateError") => NoKey::HeldAlready,
            _ => NoKey::Failed,
        }
    })?;

    written
        .as_string()
        .and_then(|written| serde_json::from_str(&written).ok())
        .ok_or(NoKey::Failed)
}
