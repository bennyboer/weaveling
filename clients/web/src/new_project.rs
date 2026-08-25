use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::workspace::Workspace;

#[component]
pub fn NewProject(workspace: Workspace) -> impl IntoView {
    let (draft, set_draft) = signal(String::new());
    let creating = workspace.creating();

    empty_the_draft_once_a_project_lands(workspace, set_draft);

    let submit = Callback::new(move |()| {
        let name = draft.get();
        if name.trim().is_empty() {
            return;
        }

        workspace.create(name);
    });

    let unusable = move || draft.get().trim().is_empty() || creating.get();

    html::div().class("new-project").child((
        html::input()
            .r#type("text")
            .placeholder("A working title…")
            .prop("value", move || draft.get())
            .on(ev::input, move |event| {
                set_draft.set(event_target_value(&event))
            })
            .on(ev::keydown, move |event| {
                if event.key() == "Enter" {
                    submit.run(());
                }
            }),
        html::button()
            .disabled(unusable)
            .on(ev::click, move |_| submit.run(()))
            .child("Create"),
    ))
}

fn empty_the_draft_once_a_project_lands(workspace: Workspace, set_draft: WriteSignal<String>) {
    let created = workspace.created();

    Effect::new(move |_| {
        if created.get().is_some() {
            set_draft.set(String::new());
        }
    });
}
