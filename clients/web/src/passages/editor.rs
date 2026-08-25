use leptos::html;
use leptos::html::Div;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use wasm_bindgen::closure::Closure;

use crate::passages::model::PassageId;
use crate::passages::prose::ProseEditor;

#[component]
pub fn PassageEditor(passage: PassageId) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let connected = RwSignal::new(false);

    let prose = StoredValue::new(None::<ProseEditor>);
    let watching = StoredValue::new_local(None::<Closure<dyn Fn(bool)>>);

    Effect::new(move || {
        let Some(surface) = host.get() else {
            return;
        };

        let on_connected = Closure::<dyn Fn(bool)>::new(move |live: bool| connected.set(live));
        prose.set_value(Some(ProseEditor::new(
            &surface,
            passage.as_str(),
            "You",
            "#f59e0b",
            &on_connected,
        )));
        watching.set_value(Some(on_connected));
    });

    on_cleanup(move || {
        prose.update_value(|prose| {
            if let Some(prose) = prose.take() {
                prose.destroy();
            }
        });
        watching.set_value(None);
    });

    html::section().class("prose").child((
        html::p()
            .class("liveness")
            .child(move || if connected.get() { "Synced" } else { "Offline" }),
        html::div().class("surface").node_ref(host),
        html::button()
            .on(ev::click, move |_| {
                prose.with_value(|prose| {
                    if let Some(prose) = prose {
                        prose.focus();
                    }
                })
            })
            .child("Focus the editor"),
    ))
}
