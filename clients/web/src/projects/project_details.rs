use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev};

use crate::http::ApiError;
use crate::icons::{Icon, mark};
use crate::projects::details_form::{DetailsDraft, details_fields};
use crate::projects::model::{DescribedProject, ProjectId};
use crate::projects::open_project::OpenProject;
use crate::projects::service;

pub fn project_details(project: ProjectId) -> impl IntoView {
    let open_project = expect_context::<OpenProject>();
    let changing = RwSignal::new(false);

    (
        html::button()
            .r#type("button")
            .class("project-details-button")
            .attr("aria-label", "Project details")
            .attr("title", "Project details")
            .disabled(move || open_project.described().is_none())
            .on(ev::click, move |_| changing.set(true))
            .child(mark(Icon::Cogwheel)),
        move || {
            changing
                .get()
                .then(|| open_project.described())
                .flatten()
                .map(|known| changing_dialog(project.clone(), known, changing, open_project))
        },
    )
}

fn changing_dialog(
    project: ProjectId,
    known: DescribedProject,
    changing: RwSignal<bool>,
    open_project: OpenProject,
) -> impl IntoView {
    let draft = DetailsDraft::of(RwSignal::new(known.name.clone()), &known.details);
    let field: NodeRef<Input> = NodeRef::new();
    let saving = RwSignal::new(false);
    let refused = RwSignal::new(None::<ApiError>);
    let project = StoredValue::new(project);
    let known = StoredValue::new(known);

    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            changing.set(false);
        }
    });
    on_cleanup(move || escaping.remove());

    let save = move || {
        let name = draft.title.get_untracked();
        if name.trim().is_empty() || saving.get_untracked() {
            return;
        }
        let details = draft.details();
        let project = project.get_value();
        let before = known.get_value();
        saving.set(true);

        spawn_local(async move {
            let renamed = match name.trim() == before.name {
                true => Ok(()),
                false => service::rename(&project, name.trim()).await.map(|_| ()),
            };
            let changed = match renamed {
                Ok(()) => service::change_details(&project, &details).await,
                Err(failure) => Err(failure),
            };

            match changed {
                Ok(described) => {
                    open_project.keep(described);
                    changing.set(false);
                }
                Err(failure) => {
                    refused.set(Some(failure));
                    saving.set(false);
                }
            }
        });
    };

    html::div()
        .class("overlay")
        .on(ev::click, move |_| changing.set(false))
        .child(
            html::div()
                .class("dialog project-details")
                .role("dialog")
                .attr("aria-modal", "true")
                .attr("aria-label", "Project details")
                .on(ev::click, |event| event.stop_propagation())
                .child((
                    html::h2().child("Project details"),
                    details_fields(draft, field, move || refused.set(None), save),
                    move || {
                        refused.get().map(|failure| {
                            html::p()
                                .class("problem")
                                .role("alert")
                                .child(failure.to_string())
                        })
                    },
                    html::div().class("dialog-actions").child((
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| changing.set(false))
                            .child("Cancel"),
                        html::button()
                            .r#type("button")
                            .class("primary")
                            .disabled(move || draft.title.get().trim().is_empty() || saving.get())
                            .on(ev::click, move |_| save())
                            .child("Save"),
                    )),
                )),
        )
}
