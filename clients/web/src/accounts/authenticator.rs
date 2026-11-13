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

/// Why the browser did not make or show a key.
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

/// Opens the browser's dialog that makes a new key.
/// The key is made with the options the server asked for.
pub async fn made(asked: &Value) -> Result<Value, NoKey> {
    answered(make_key(&asked.to_string()).await)
}

/// Opens the browser's dialog that asks the user to touch a key.
/// The dialog uses the options the server asked for.
pub async fn touched(asked: &Value) -> Result<Value, NoKey> {
    answered(touch_key(&asked.to_string()).await)
}

/// Whether the browser can offer its passkeys among the suggestions for the address field.
pub async fn offerable() -> bool {
    passkeys_offerable().await.as_bool() == Some(true)
}

/// Offers the browser's passkeys for this site among the suggestions for the address field.
/// Returns the passkey the user picked.
pub async fn offered(asked: &Value) -> Result<Value, NoKey> {
    answered(offer_passkeys(&asked.to_string()).await)
}

/// Stops offering passkeys in the address field.
/// This lets the browser ask for a key in another way.
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
