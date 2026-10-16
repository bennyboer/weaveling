use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::projects::languages::{BOOK_LANGUAGES, the_browser_s};
use crate::projects::model::ProjectDetails;
use crate::projects::workspace::Workspace;

#[component]
pub fn NewProject(workspace: Workspace) -> impl IntoView {
    let (draft, set_draft) = signal(String::new());
    let asking = RwSignal::new(false);
    let creating = workspace.creating();

    empty_the_draft_once_a_project_lands(workspace, set_draft, asking);

    let ask = move || {
        if !draft.get_untracked().trim().is_empty() {
            asking.set(true);
        }
    };

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
                    ask();
                }
            }),
        html::button()
            .disabled(unusable)
            .on(ev::click, move |_| ask())
            .child("Create"),
        move || {
            asking
                .get()
                .then(|| new_project_dialog(workspace, draft, set_draft, asking))
        },
    ))
}

fn new_project_dialog(
    workspace: Workspace,
    draft: ReadSignal<String>,
    set_draft: WriteSignal<String>,
    asking: RwSignal<bool>,
) -> impl IntoView {
    let title: NodeRef<Input> = NodeRef::new();
    let language = RwSignal::new(the_browser_s().unwrap_or_default().to_owned());
    let description = RwSignal::new(String::new());
    let creating = workspace.creating();
    let attempted = RwSignal::new(false);
    let refused = workspace.refused_creation();

    Effect::new(move || {
        if let Some(field) = title.get() {
            let _ = field.focus();
            field.select();
        }
    });
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            asking.set(false);
        }
    });
    on_cleanup(move || escaping.remove());

    let create = move || {
        let name = draft.get_untracked();
        if name.trim().is_empty() {
            return;
        }

        attempted.set(true);
        workspace.create(
            name,
            ProjectDetails {
                language: Some(language.get_untracked()).filter(|chosen| !chosen.is_empty()),
                description: description.get_untracked(),
            },
        );
    };

    html::div()
        .class("overlay")
        .on(ev::click, move |_| asking.set(false))
        .child(
            html::div()
                .class("dialog project-details")
                .role("dialog")
                .attr("aria-modal", "true")
                .attr("aria-label", "New project")
                .on(ev::click, |event| event.stop_propagation())
                .child((
                    html::h2().child("New project"),
                    html::label().class("field").child((
                        html::span().child("Title"),
                        html::input()
                            .r#type("text")
                            .node_ref(title)
                            .prop("value", move || draft.get())
                            .on(ev::input, move |event| {
                                attempted.set(false);
                                set_draft.set(event_target_value(&event))
                            })
                            .on(ev::keydown, move |event| {
                                if event.key() == "Enter" {
                                    create();
                                }
                            }),
                    )),
                    html::label().class("field").child((
                        html::span().child("Language of the book"),
                        html::select()
                            .on(ev::change, move |event| {
                                language.set(event_target_value(&event))
                            })
                            .child((
                                html::option()
                                    .value("")
                                    .prop("selected", move || language.get().is_empty())
                                    .child("Not set"),
                                BOOK_LANGUAGES
                                    .iter()
                                    .map(|(tag, named)| {
                                        html::option()
                                            .value(*tag)
                                            .prop("selected", move || language.get() == *tag)
                                            .child(*named)
                                    })
                                    .collect::<Vec<_>>(),
                            )),
                        html::small().child(
                            "The language the book is written in, used wherever its text depends on it.",
                        ),
                    )),
                    html::label().class("field").child((
                        html::span().child("Description"),
                        html::textarea()
                            .attr("rows", "3")
                            .attr("placeholder", "What the book is about, in a few lines. Optional.")
                            .prop("value", move || description.get())
                            .on(ev::input, move |event| {
                                description.set(event_target_value(&event))
                            }),
                    )),
                    move || {
                        attempted
                            .get()
                            .then(|| refused.get())
                            .flatten()
                            .map(|failure| {
                                html::p()
                                    .class("problem")
                                    .role("alert")
                                    .child(failure.to_string())
                            })
                    },
                    html::div().class("dialog-actions").child((
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| asking.set(false))
                            .child("Cancel"),
                        html::button()
                            .r#type("button")
                            .class("primary")
                            .disabled(move || draft.get().trim().is_empty() || creating.get())
                            .on(ev::click, move |_| create())
                            .child("Create project"),
                    )),
                )),
        )
}

fn empty_the_draft_once_a_project_lands(
    workspace: Workspace,
    set_draft: WriteSignal<String>,
    asking: RwSignal<bool>,
) {
    let created = workspace.created();

    Effect::new(move |_| {
        if created.get().is_some() {
            set_draft.set(String::new());
            asking.set(false);
        }
    });
}
