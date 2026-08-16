mod bridge;
mod workbench;

use leptos::prelude::*;

use crate::workbench::Workbench;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        let mounted = RwSignal::new(true);

        view! {
            <main>
                <h1>"Rung 4 — Leptos, ProseMirror and Yjs"</h1>
                <p class="tagline">
                    "Two editors, no server. Every keystroke becomes a Yjs update that crosses \
                     into Rust, lands in a yrs replica, and is handed to the other editor."
                </p>
                <button on:click=move |_| mounted.update(|on| *on = !*on)>
                    {move || if mounted.get() { "Unmount" } else { "Mount" }}
                </button>
                <Show when=move || mounted.get()>
                    <Workbench />
                </Show>
            </main>
        }
    });
}
