use leptos::logging::warn;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::HtmlElement;

#[wasm_bindgen(raw_module = "/js/prose.js")]
extern "C" {
    #[wasm_bindgen(js_name = editorLoaded, catch)]
    async fn editor_fetched() -> Result<JsValue, JsValue>;

    pub type SyncConnection;

    #[wasm_bindgen(constructor)]
    pub fn new() -> SyncConnection;

    #[wasm_bindgen(method)]
    pub fn close(this: &SyncConnection);

    #[wasm_bindgen(method, js_name = setPeople)]
    pub fn set_people(this: &SyncConnection, told: &str);

    pub type ProseEditor;

    #[wasm_bindgen(constructor)]
    pub fn new(
        host: &HtmlElement,
        scene: &str,
        account: &str,
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

    #[wasm_bindgen(method, js_name = onPresent)]
    pub fn on_present(this: &ProseEditor, present: &Closure<dyn Fn(String)>);

    #[wasm_bindgen(method, js_name = onSelectionShown)]
    pub fn on_selection_shown(this: &ProseEditor, shown: &Closure<dyn Fn(String)>);

    #[wasm_bindgen(method, js_name = onToToolbar)]
    pub fn on_to_toolbar(this: &ProseEditor, reached: &Closure<dyn Fn()>);

    #[wasm_bindgen(method, js_name = toggleBold)]
    pub fn toggle_bold(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = toggleItalic)]
    pub fn toggle_italic(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = quoteSelection)]
    pub fn quote_selection(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = clearFormatting)]
    pub fn clear_formatting(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = onLinkAsked)]
    pub fn on_link_asked(this: &ProseEditor, asked: &Closure<dyn Fn()>);

    #[wasm_bindgen(method, js_name = setLink)]
    pub fn set_link(this: &ProseEditor, typed: &str) -> bool;

    #[wasm_bindgen(method, js_name = removeLink)]
    pub fn remove_link(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = onImagesGiven)]
    pub fn on_images_given(this: &ProseEditor, given: &Closure<dyn Fn(js_sys::Array, u32)>);

    #[wasm_bindgen(method, js_name = onFiguresPasted)]
    pub fn on_figures_pasted(this: &ProseEditor, pasted: &Closure<dyn Fn(js_sys::Array)>);

    #[wasm_bindgen(method, js_name = replaceAsset)]
    pub fn replace_asset(this: &ProseEditor, old: &str, copy: &str);

    #[wasm_bindgen(method, js_name = onFiguresCopied)]
    pub fn on_figures_copied(this: &ProseEditor, copied: &Closure<dyn Fn(js_sys::Array)>);

    #[wasm_bindgen(method, js_name = leaveOut)]
    pub fn leave_out(this: &ProseEditor, asset: &str);

    #[wasm_bindgen(method, js_name = setLabels)]
    pub fn set_labels(this: &ProseEditor, told: &str);

    #[wasm_bindgen(method, js_name = askedForImage)]
    pub fn asked_for_image(this: &ProseEditor) -> u32;

    #[wasm_bindgen(method, js_name = insertFigure)]
    pub fn insert_figure(this: &ProseEditor, asset: &str, ticket: u32);

    #[wasm_bindgen(method, js_name = insertChosen)]
    pub fn insert_chosen(this: &ProseEditor, asset: &str, ticket: u32);

    #[wasm_bindgen(method, js_name = forgetWaiting)]
    pub fn forget_waiting(this: &ProseEditor, ticket: u32);

    #[wasm_bindgen(method, js_name = uploadStarted)]
    pub fn upload_started(this: &ProseEditor, ticket: u32, nth: u32, name: &str, label: &str);

    #[wasm_bindgen(method, js_name = uploadProgress)]
    pub fn upload_progress(this: &ProseEditor, ticket: u32, nth: u32, fraction: f64);

    #[wasm_bindgen(method, js_name = uploadRefused)]
    pub fn upload_refused(this: &ProseEditor, ticket: u32, nth: u32, told: &str);

    #[wasm_bindgen(method, js_name = uploadDone)]
    pub fn upload_done(this: &ProseEditor, ticket: u32, nth: u32);

    #[wasm_bindgen(method, js_name = setFigureSize)]
    pub fn set_figure_size(this: &ProseEditor, size: &str);

    #[wasm_bindgen(method, js_name = removeFigure)]
    pub fn remove_figure(this: &ProseEditor);

    #[wasm_bindgen(method, js_name = arriveWhenSynced)]
    pub fn arrive_when_synced(this: &ProseEditor, towards: &str);

    #[wasm_bindgen(method, js_name = landAt)]
    pub fn land_at(this: &ProseEditor, at: u32);

    #[wasm_bindgen(method, js_name = landOnAsset)]
    pub fn land_on_asset(this: &ProseEditor, asset: &str);

    #[wasm_bindgen(method, js_name = onSaving)]
    pub fn on_saving(this: &ProseEditor, saving: &Closure<dyn Fn(bool)>);

    #[wasm_bindgen(method, js_name = onLeaving)]
    pub fn on_leaving(this: &ProseEditor, leaving: &Closure<dyn Fn(String) -> bool>);

    #[wasm_bindgen(method, js_name = writtenText)]
    pub fn written_text(this: &ProseEditor) -> String;

    #[wasm_bindgen(method, js_name = setQuotationMarks)]
    pub fn set_quotation_marks(this: &ProseEditor, opening: &str, closing: &str);

    #[wasm_bindgen(method, js_name = onSelecting)]
    pub fn on_selecting(this: &ProseEditor, selecting: &Closure<dyn Fn(String) -> bool>);

}

/// Fetches the editor code, ProseMirror and Yjs.
/// Only the writing view needs it.
/// It must be fetched before anything makes a `ProseEditor` or a `SyncConnection`.
pub async fn editor_loaded() {
    if let Err(failed) = editor_fetched().await {
        warn!("the editor could not be fetched: {failed:?}");
    }
}
