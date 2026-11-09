use serde_json::Value;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(raw_module = "/js/shell.js")]
extern "C" {
    #[wasm_bindgen(js_name = keysSupported)]
    fn keys_supported() -> bool;

    #[wasm_bindgen(js_name = makeKey, catch)]
    async fn make_key(asked: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = passkeysOfferable)]
    async fn passkeys_offerable() -> JsValue;

    #[wasm_bindgen(js_name = offerPasskeys, catch)]
    async fn offer_passkeys(asked: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = stopOfferingPasskeys)]
    fn stop_offering_passkeys();

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

/// Whether the browser can offer its passkeys where it fills in the address.
pub async fn offerable() -> bool {
    passkeys_offerable().await.as_bool() == Some(true)
}

/// The passkeys the browser holds for this site, offered among what it fills in the address, the one picked.
pub async fn offered(asked: &Value) -> Result<Value, NoKey> {
    answered(offer_passkeys(&asked.to_string()).await)
}

/// Takes back the passkeys offered in the address field, so the browser can ask otherwise.
pub fn stop_offering() {
    stop_offering_passkeys();
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
