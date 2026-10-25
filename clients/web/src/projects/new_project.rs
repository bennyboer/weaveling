use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev};
use leptos_router::NavigateOptions;
use leptos_router::hooks;

use crate::dialog;
use crate::problem;
use crate::projects::details_form;
use crate::projects::details_form::DetailsDraft;
use crate::projects::languages;
use crate::projects::model::{ProjectDetails, ProjectId};
use crate::projects::workspace::Workspace;
use crate::route;
use crate::speech;

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
            .placeholder(speech::said("new-project-working-title"))
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
            .child(speech::said("new-project-create")),
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
            language: languages::the_browser_s().map(str::to_owned),
            quotation: None,
            description: String::new(),
        },
    );
    let title: NodeRef<Input> = NodeRef::new();
    let creating = workspace.creating();
    let setting_up = workspace.setting_up();
    let attempted = RwSignal::new(false);
    let refused = workspace.refused_creation();

    let create = move || {
        let name = draft.get_untracked();
        if name.trim().is_empty() {
            return;
        }

        attempted.set(true);
        workspace.create(name, details.details());
    };

    dialog::modal(
        Signal::derive(speech::said("new-project-dialog")),
        "project-details",
        move || asking.set(false),
        (
            details_form::details_fields(details, title, move || attempted.set(false), create),
            move || {
                setting_up.get().then(|| {
                    html::p()
                        .class("hint")
                        .role("status")
                        .child(speech::said("new-project-setting-up"))
                })
            },
            move || {
                attempted
                    .get()
                    .then(|| refused.get())
                    .flatten()
                    .map(problem::problem)
            },
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| asking.set(false))
                    .child(speech::said("new-project-cancel")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .disabled(move || draft.get().trim().is_empty() || creating.get())
                    .on(ev::click, move |_| create())
                    .child(speech::said("new-project-confirm")),
            )),
        ),
    )
}

fn go_into_a_project_once_it_lands(
    workspace: Workspace,
    draft: RwSignal<String>,
    asking: RwSignal<bool>,
) {
    let created = workspace.created();
    let navigate = hooks::use_navigate();

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
