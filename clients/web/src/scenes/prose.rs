use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::HtmlElement;

#[wasm_bindgen(module = "/js/editor.bundle.js")]
extern "C" {
    pub type SyncConnection;

    #[wasm_bindgen(constructor)]
    pub fn new() -> SyncConnection;

    #[wasm_bindgen(method)]
    pub fn close(this: &SyncConnection);

    pub type ProseEditor;

    #[wasm_bindgen(constructor)]
    pub fn new(
        host: &HtmlElement,
        scene: &str,
        author: &str,
        color: &str,
        on_connected: &Closure<dyn Fn(bool)>,
        connection: &SyncConnection,
    ) -> ProseEditor;

    #[wasm_bindgen(method)]
    pub fn destroy(this: &ProseEditor);

    #[wasm_bindgen(method)]
    pub fn focus(this: &ProseEditor);

    #[wasm_bindgen(method)]
    pub fn seed(this: &ProseEditor, text: &str);

    #[wasm_bindgen(method, js_name = focusStart)]
    pub fn focus_start(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = focusEnd)]
    pub fn focus_end(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = onChanged)]
    pub fn on_changed(this: &ProseEditor, changed: &Closure<dyn Fn(String)>);

    #[wasm_bindgen(method, js_name = onBreaking)]
    pub fn on_breaking(this: &ProseEditor, breaking: &Closure<dyn Fn()>);

    #[wasm_bindgen(method, js_name = onCaretMoved)]
    pub fn on_caret_moved(this: &ProseEditor, moved: &Closure<dyn Fn(u32)>);

    #[wasm_bindgen(method, js_name = arriveWhenSynced)]
    pub fn arrive_when_synced(this: &ProseEditor, towards: &str);

    #[wasm_bindgen(method, js_name = landAt)]
    pub fn land_at(this: &ProseEditor, at: u32);

    #[wasm_bindgen(method, js_name = onSaving)]
    pub fn on_saving(this: &ProseEditor, saving: &Closure<dyn Fn(bool)>);

    #[wasm_bindgen(method, js_name = onLeaving)]
    pub fn on_leaving(this: &ProseEditor, leaving: &Closure<dyn Fn(String) -> bool>);

    #[wasm_bindgen(method, js_name = setQuotationMarks)]
    pub fn set_quotation_marks(this: &ProseEditor, opening: &str, closing: &str);

    #[wasm_bindgen(method, js_name = onSelecting)]
    pub fn on_selecting(this: &ProseEditor, selecting: &Closure<dyn Fn(String) -> bool>);

    #[wasm_bindgen(method, js_name = plainText)]
    pub fn plain_text(this: &ProseEditor) -> String;
}
