use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev};
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;

use crate::projects::details_form::{DetailsDraft, details_fields};
use crate::projects::languages::the_browser_s;
use crate::projects::model::{ProjectDetails, ProjectId};
use crate::projects::workspace::Workspace;
use crate::route;

#[component]
pub fn NewProject(workspace: Workspace) -> impl IntoView {
    let draft = RwSignal::new(String::new());
    let asking = RwSignal::new(false);
    let creating = workspace.creating();

    go_into_a_project_once_it_lands(workspace, draft, asking);

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
                draft.set(event_target_value(&event))
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
                .then(|| new_project_dialog(workspace, draft, asking))
        },
    ))
}

fn new_project_dialog(
    workspace: Workspace,
    draft: RwSignal<String>,
    asking: RwSignal<bool>,
) -> impl IntoView {
    let details = DetailsDraft::of(
        draft,
        &ProjectDetails {
            language: the_browser_s().map(str::to_owned),
            quotation: None,
            description: String::new(),
        },
    );
    let title: NodeRef<Input> = NodeRef::new();
    let creating = workspace.creating();
    let attempted = RwSignal::new(false);
    let refused = workspace.refused_creation();

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
        workspace.create(name, details.details());
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
                    details_fields(details, title, move || attempted.set(false), create),
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

fn go_into_a_project_once_it_lands(
    workspace: Workspace,
    draft: RwSignal<String>,
    asking: RwSignal<bool>,
) {
    let created = workspace.created();
    let navigate = use_navigate();

    Effect::new(move |before: Option<Option<ProjectId>>| {
        let landed = created.get();
        let now = landed.as_ref().map(|project| project.id.clone());

        if let (Some(before), Some(project)) = (before, landed)
            && before.as_ref() != Some(&project.id)
        {
            draft.set(String::new());
            asking.set(false);
            navigate(
                &route::project(&project.id, &project.name),
                NavigateOptions::default(),
            );
        }

        now
    });
}
