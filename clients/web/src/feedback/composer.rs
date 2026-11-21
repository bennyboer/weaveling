use feedback_contract::CommentBlockDTO;
use leptos::logging::warn;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::HtmlElement;

use crate::feedback::model::CommentBlock;
use crate::icons;
use crate::icons::Icon;
use crate::speech;

#[wasm_bindgen(raw_module = "/js/comment.js")]
extern "C" {
    #[wasm_bindgen(js_name = commentEditorLoaded, catch)]
    async fn comment_editor_fetched() -> Result<JsValue, JsValue>;

    pub type CommentEditor;

    #[wasm_bindgen(constructor)]
    pub fn new(
        host: &HtmlElement,
        held: &str,
        changed: &Closure<dyn Fn(bool)>,
        save: &Closure<dyn Fn()>,
    ) -> CommentEditor;

    #[wasm_bindgen(method)]
    pub fn written(this: &CommentEditor) -> String;

    #[wasm_bindgen(method, js_name = saysNothing)]
    pub fn says_nothing(this: &CommentEditor) -> bool;

    #[wasm_bindgen(method, js_name = toggleBold)]
    pub fn toggle_bold(this: &CommentEditor);

    #[wasm_bindgen(method, js_name = toggleItalic)]
    pub fn toggle_italic(this: &CommentEditor);

    #[wasm_bindgen(method, js_name = hasSelection)]
    pub fn has_selection(this: &CommentEditor) -> bool;

    #[wasm_bindgen(method, js_name = setLink)]
    pub fn set_link(this: &CommentEditor, typed: &str) -> bool;

    #[wasm_bindgen(method)]
    pub fn focus(this: &CommentEditor);

    #[wasm_bindgen(method)]
    pub fn destroy(this: &CommentEditor);
}

/// The editor as the composer holds it, with what it calls back, which must live as long as it does.
struct Held {
    editor: CommentEditor,
    _changed: Closure<dyn Fn(bool)>,
    _save: Closure<dyn Fn()>,
}

/// Where a comment is written: the editor of the reading view's own, fetched the first time a comment is begun,
/// with a toolbar for bold, italics and a link, and a field for a link's address.
/// What is written is handed over as blocks, never as markup.
pub fn composer(
    held: Vec<CommentBlock>,
    saved_as: &'static str,
    saving: Signal<bool>,
    save: impl Fn(Vec<CommentBlock>) + Copy + Send + Sync + 'static,
    cancel: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let host = NodeRef::<html::Div>::new();
    let editor = StoredValue::new_local(None::<Held>);
    let ready = RwSignal::new(false);
    let blank = RwSignal::new(true);
    let linking = RwSignal::new(false);
    let held = serde_json::to_string(&held.iter().map(CommentBlockDTO::from).collect::<Vec<_>>())
        .unwrap_or_else(|_| "[]".to_owned());
    let saved = move || {
        let written = editor.with_value(|held| {
            held.as_ref()
                .filter(|held| !held.editor.says_nothing())
                .map(|held| held.editor.written())
        });
        let Some(written) = written else {
            return;
        };
        match serde_json::from_str::<Vec<CommentBlockDTO>>(&written) {
            Ok(blocks) => save(blocks.into_iter().map(CommentBlock::from).collect()),
            Err(failed) => warn!("the comment written could not be read: {failed}"),
        }
    };
    host.on_load(move |host| {
        leptos::task::spawn_local(async move {
            if let Err(failed) = comment_editor_fetched().await {
                warn!("the comment editor could not be fetched: {failed:?}");
                return;
            }
            let changed: Closure<dyn Fn(bool)> =
                Closure::new(move |nothing: bool| _ = blank.try_set(nothing));
            let save: Closure<dyn Fn()> = Closure::new(saved);
            let made = CommentEditor::new(&host, &held, &changed, &save);
            blank.set(made.says_nothing());
            made.focus();
            editor.set_value(Some(Held {
                editor: made,
                _changed: changed,
                _save: save,
            }));
            ready.set(true);
        });
    });
    on_cleanup(move || {
        if let Some(held) = editor.try_update_value(Option::take).flatten() {
            held.editor.destroy();
        }
    });
    let told = move |act: fn(&CommentEditor)| {
        editor.with_value(|held| {
            if let Some(held) = held {
                act(&held.editor);
                held.editor.focus();
            }
        });
    };

    html::div()
        .class("composer")
        .on(ev::keydown, move |event| {
            if event.key() == "Escape" && !linking.get_untracked() {
                event.prevent_default();
                event.stop_propagation();
                cancel();
            }
        })
        .child((
            move || match linking.get() {
                false => toolbar(editor, told, linking).into_any(),
                true => link_field(editor, linking).into_any(),
            },
            html::div()
                .node_ref(host)
                .class("comment-host")
                .attr("data-label", speech::said("feedback-your-comment"))
                .attr(
                    "data-placeholder",
                    speech::said("feedback-your-comment-placeholder"),
                )
                .attr("data-blank", move || blank.get().to_string()),
            html::div().class("composer-actions").child((
                html::button()
                    .r#type("button")
                    .class("primary")
                    .disabled(move || !ready.get() || blank.get() || saving.get())
                    .on(ev::click, move |_| saved())
                    .child(speech::said(saved_as)),
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| cancel())
                    .child(speech::said("cancel")),
            )),
        ))
}

fn toolbar(
    editor: StoredValue<Option<Held>, LocalStorage>,
    told: impl Fn(fn(&CommentEditor)) + Copy + 'static,
    linking: RwSignal<bool>,
) -> impl IntoView {
    let button = move |icon: Icon, label: &'static str, act: fn(&CommentEditor)| {
        html::button()
            .r#type("button")
            .attr("aria-label", speech::said(label))
            .attr("title", speech::said(label))
            .on(ev::pointerdown, |event| event.prevent_default())
            .on(ev::click, move |_| told(act))
            .child(icons::mark(icon))
    };

    html::div()
        .class("composer-toolbar")
        .role("toolbar")
        .attr("aria-label", speech::said("formatting"))
        .child((
            button(Icon::Bold, "formatting-bold", CommentEditor::toggle_bold),
            button(
                Icon::Italic,
                "formatting-italic",
                CommentEditor::toggle_italic,
            ),
            html::button()
                .r#type("button")
                .attr("aria-label", speech::said("formatting-link"))
                .attr("title", speech::said("formatting-link"))
                .on(ev::pointerdown, |event| event.prevent_default())
                .on(ev::click, move |_| {
                    let selected = editor.with_value(|held| {
                        held.as_ref()
                            .is_some_and(|held| held.editor.has_selection())
                    });
                    if selected {
                        linking.set(true);
                    }
                })
                .child(icons::mark(Icon::Link)),
        ))
}

/// The field for a link's address, in place of the toolbar, linking what is selected in the comment.
/// An address without a scheme is taken as a web address, or as an email address if it looks like one.
fn link_field(
    editor: StoredValue<Option<Held>, LocalStorage>,
    linking: RwSignal<bool>,
) -> impl IntoView {
    let address = RwSignal::new(String::new());
    let refused = RwSignal::new(false);
    let typed = NodeRef::<html::Input>::new();
    typed.on_load(|input| _ = input.focus());
    let back = move || {
        linking.set(false);
        editor.with_value(|held| {
            if let Some(held) = held {
                held.editor.focus();
            }
        });
    };

    html::form()
        .class("composer-linking")
        .on(ev::submit, move |event| {
            event.prevent_default();
            let typed = address.get_untracked();
            let linked = editor.with_value(|held| {
                held.as_ref()
                    .is_some_and(|held| held.editor.set_link(&typed))
            });
            if linked {
                back();
            } else {
                refused.set(true);
            }
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Escape" {
                event.prevent_default();
                event.stop_propagation();
                back();
            }
        })
        .child((
            html::input()
                .node_ref(typed)
                .r#type("text")
                .attr("inputmode", "url")
                .attr("aria-label", speech::said("formatting-link-address"))
                .attr("placeholder", speech::said("formatting-link-address"))
                .on(ev::input, move |event| {
                    address.set(event_target_value(&event))
                }),
            html::button()
                .r#type("submit")
                .child(speech::said("formatting-link-set")),
            move || {
                refused.get().then(|| {
                    html::p()
                        .class("refused")
                        .role("alert")
                        .child(speech::said("formatting-link-refused"))
                })
            },
        ))
}
