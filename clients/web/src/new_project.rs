use leptos::prelude::*;

use crate::workspace::Workspace;

#[component]
pub fn NewProject(workspace: Workspace) -> impl IntoView {
    let (draft, set_draft) = signal(String::new());

    let submit = Callback::new(move |()| {
        let name = draft.get();
        if name.trim().is_empty() {
            return;
        }

        workspace.create(name, move || set_draft.set(String::new()));
    });

    view! {
        <div class="new-project">
            <input
                type="text"
                placeholder="A working title…"
                prop:value=move || draft.get()
                on:input=move |event| set_draft.set(event_target_value(&event))
                on:keydown=move |event| {
                    if event.key() == "Enter" {
                        submit.run(());
                    }
                }
            />
            <button
                disabled=move || draft.get().trim().is_empty()
                on:click=move |_| submit.run(())
            >
                "Create"
            </button>
        </div>
    }
}
