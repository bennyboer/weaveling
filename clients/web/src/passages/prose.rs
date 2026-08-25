use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::HtmlElement;

#[wasm_bindgen(module = "/js/editor.bundle.js")]
extern "C" {
    pub type ProseEditor;

    #[wasm_bindgen(constructor)]
    pub fn new(
        host: &HtmlElement,
        passage: &str,
        author: &str,
        color: &str,
        on_connected: &Closure<dyn Fn(bool)>,
    ) -> ProseEditor;

    #[wasm_bindgen(method)]
    pub fn destroy(this: &ProseEditor);

    #[wasm_bindgen(method)]
    pub fn focus(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = plainText)]
    pub fn plain_text(this: &ProseEditor) -> String;
}
