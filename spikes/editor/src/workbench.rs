use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::closure::Closure;
use crdt_spike::projection::{outline, plain_text};
use crdt_spike::{PROSE, absorb, doc_for, state_vector, whats_missing};
use yrs::types::xml::XmlOut;
use yrs::{Doc, Text, Transact, XmlFragment};

use crate::bridge::ProseEditor;

type Editor = StoredValue<Option<ProseEditor>>;

fn write_from_rust(doc: &Doc, chunk: &str) -> Vec<u8> {
    let seen = state_vector(doc);
    let fragment = doc.get_or_insert_xml_fragment(PROSE);

    let paragraph = {
        let txn = doc.transact();
        fragment.children(&txn).find_map(|node| match node {
            XmlOut::Element(element) if element.tag().as_ref() == "paragraph" => Some(element),
            _ => None,
        })
    };

    if let Some(paragraph) = paragraph {
        let prose = {
            let txn = doc.transact();
            match paragraph.children(&txn).next() {
                Some(XmlOut::Text(text)) => Some(text),
                _ => None,
            }
        };

        if let Some(prose) = prose {
            let mut txn = doc.transact_mut();
            prose.insert(&mut txn, 0, chunk);
        }
    }

    whats_missing(doc, &seen)
}

#[component]
pub fn Workbench() -> impl IntoView {
    let left_host: NodeRef<Div> = NodeRef::new();
    let right_host: NodeRef<Div> = NodeRef::new();

    let left: Editor = StoredValue::new(None);
    let right: Editor = StoredValue::new(None);
    let server = StoredValue::new_local(doc_for(1));
    let wiring = StoredValue::new_local(Vec::<Closure<dyn Fn(Vec<u8>)>>::new());

    let relayed = RwSignal::new(0usize);
    let bytes = RwSignal::new(0usize);
    let prose = RwSignal::new(String::new());
    let shape = RwSignal::new(String::new());
    let agree = RwSignal::new(true);

    let severed = RwSignal::new(false);
    let pending = RwSignal::new(0usize);
    let queue = StoredValue::new_local(Vec::<(Editor, Vec<u8>)>::new());

    let refresh = move || {
        let (text, blocks) =
            server.with_value(|doc| (plain_text(doc), outline(doc).join(", ")));
        let mirrors = [left, right].map(|editor| {
            editor.with_value(|editor| editor.as_ref().map(|editor| (editor.plain_text(), editor.valid())))
        });

        agree.set(
            mirrors
                .iter()
                .flatten()
                .all(|(mirror, valid)| *valid && *mirror == text),
        );
        prose.set(text);
        shape.set(blocks);
    };

    let deliver = move |to: Editor, update: &[u8]| {
        server.with_value(|doc| absorb(doc, update));
        to.with_value(|editor| {
            if let Some(editor) = editor {
                editor.absorb(update);
            }
        });

        relayed.update(|count| *count += 1);
        bytes.update(|total| *total += update.len());
    };

    let relay = move |to: Editor, update: Vec<u8>| {
        if severed.get_untracked() {
            queue.update_value(|queued| queued.push((to, update)));
            pending.set(queue.with_value(Vec::len));
        } else {
            deliver(to, &update);
        }

        refresh();
    };

    let reconnect = move |_| {
        let queued = queue.with_value(Clone::clone);
        queue.set_value(Vec::new());

        for (to, update) in &queued {
            deliver(*to, update);
        }

        severed.set(false);
        pending.set(0);
        refresh();
    };

    Effect::new(move || {
        let (Some(left_el), Some(right_el)) = (left_host.get(), right_host.get()) else {
            return;
        };

        let to_right = Closure::<dyn Fn(Vec<u8>)>::new(move |update| relay(right, update));
        let to_left = Closure::<dyn Fn(Vec<u8>)>::new(move |update| relay(left, update));

        right.set_value(Some(ProseEditor::new(
            &right_el, 200.0, "Bo", "#38bdf8", false, &to_left,
        )));
        left.set_value(Some(ProseEditor::new(
            &left_el, 100.0, "Ada", "#f59e0b", true, &to_right,
        )));

        wiring.set_value(vec![to_right, to_left]);
        refresh();
    });

    on_cleanup(move || {
        for editor in [left, right] {
            editor.update_value(|editor| {
                if let Some(editor) = editor.take() {
                    editor.destroy();
                }
            });
        }
        wiring.set_value(Vec::new());
    });

    let write = move |_| {
        let update = server.with_value(|doc| write_from_rust(doc, "Even now, "));
        for editor in [left, right] {
            editor.with_value(|editor| {
                if let Some(editor) = editor {
                    editor.absorb(&update);
                }
            });
        }

        relayed.update(|count| *count += 1);
        bytes.update(|total| *total += update.len());
        refresh();
    };

    view! {
        <div class="pair">
            <section>
                <h2>"Ada"</h2>
                <div class="surface" node_ref=left_host></div>
            </section>
            <section>
                <h2>"Bo"</h2>
                <div class="surface" node_ref=right_host></div>
            </section>
        </div>

        <section class="projection">
            <h2>
                "What the Rust replica sees"
                <span class:agree=move || agree.get() class:diverged=move || !agree.get()>
                    {move || if agree.get() { "all three agree" } else { "diverged" }}
                </span>
            </h2>
            <dl>
                <dt>"updates relayed"</dt>
                <dd>{move || relayed.get()}</dd>
                <dt>"bytes relayed"</dt>
                <dd>{move || bytes.get()}</dd>
                <dt>"queued"</dt>
                <dd>{move || pending.get()}</dd>
                <dt>"outline"</dt>
                <dd>{move || shape.get()}</dd>
                <dt>"characters"</dt>
                <dd>{move || prose.get().chars().count()}</dd>
            </dl>
            <pre>{move || prose.get()}</pre>
            <div class="controls">
                <button on:click=write>"Write into the document from Rust"</button>
                <Show
                    when=move || severed.get()
                    fallback=move || {
                        view! {
                            <button on:click=move |_| severed.set(true)>"Sever the link"</button>
                        }
                    }
                >
                    <button on:click=reconnect>"Reconnect and merge"</button>
                </Show>
            </div>
        </section>
    }
}
