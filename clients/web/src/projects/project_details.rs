use std::time::Duration;

use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev};
use quotation::QuotationMarks;

use crate::dialog::modal;
use crate::http::ApiError;
use crate::icons::{Icon, mark};
use crate::problem::problem;
use crate::projects::details_form::{DetailsDraft, details_fields};
use crate::projects::model::{DescribedProject, ProjectId, Version};
use crate::projects::open_project::OpenProject;
use crate::projects::service;
use crate::scenes::restyling::{Restyled, standing};
use crate::scenes::service as scenes;

const FOLLOW_EVERY: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Editing,
    Confirming {
        from: QuotationMarks,
        to: QuotationMarks,
    },
    Restyling {
        to: QuotationMarks,
    },
}

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
                .then(|| untrack(|| open_project.described()))
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
    let stage = RwSignal::new(Stage::Editing);
    let restyled = RwSignal::new(Restyled::Waiting);
    let project = StoredValue::new(project);
    let known = StoredValue::new(known);

    let save = move |confirmed: bool| {
        let name = draft.title.get_untracked();
        if name.trim().is_empty() || saving.get_untracked() {
            return;
        }
        let details = draft.details();
        let project = project.get_value();
        let before = known.get_value();
        let (from, to) = (before.details.quotation_marks(), details.quotation_marks());
        if from != to && !confirmed {
            stage.set(Stage::Confirming { from, to });
            return;
        }
        saving.set(true);

        spawn_local(async move {
            let renamed = match name.trim() == before.name {
                true => Ok(before.version),
                false => service::rename(&project, name.trim())
                    .await
                    .map(|renamed| renamed.version),
            };
            let changed = match renamed {
                Ok(version) => service::change_details(&project, &details, version)
                    .await
                    .map(|described| (described, version)),
                Err(failure) => Err(failure),
            };

            match changed {
                Ok((described, _)) if from == to => {
                    open_project.keep(described);
                    changing.set(false);
                }
                Ok((described, saved_against)) => {
                    open_project.keep(described);
                    stage.set(Stage::Restyling { to });
                    follow(project, saved_against, restyled);
                }
                Err(failure) => {
                    refused.set(Some(failure));
                    saving.set(false);
                    stage.set(Stage::Editing);
                }
            }
        });
    };

    modal(
        "Project details",
        "project-details",
        move || changing.set(false),
        move || match stage.get() {
            Stage::Editing => (
                details_fields(draft, field, move || refused.set(None), move || save(false)),
                move || refused.get().map(problem),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| changing.set(false))
                        .child("Cancel"),
                    html::button()
                        .r#type("button")
                        .class("primary")
                        .disabled(move || draft.title.get().trim().is_empty() || saving.get())
                        .on(ev::click, move |_| save(false))
                        .child("Save"),
                )),
            )
                .into_any(),
            Stage::Confirming { from, to } => (
                html::p().child(format!(
                    "Changing the quotation marks from {} to {} rewrites them in every scene of \
                     the book. In a long book that takes a while.",
                    from.shown(),
                    to.shown()
                )),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| stage.set(Stage::Editing))
                        .child("Back"),
                    html::button()
                        .r#type("button")
                        .class("primary")
                        .disabled(move || saving.get())
                        .on(ev::click, move |_| save(true))
                        .child("Change the marks"),
                )),
            )
                .into_any(),
            Stage::Restyling { to } => restyling(to, restyled, changing).into_any(),
        },
    )
}

fn restyling(
    to: QuotationMarks,
    restyled: RwSignal<Restyled>,
    changing: RwSignal<bool>,
) -> impl IntoView {
    (
        html::p()
            .attr("aria-live", "polite")
            .child(move || match restyled.get() {
                Restyled::Waiting => "Rewriting the quotation marks…".to_owned(),
                Restyled::Running { done, total } => {
                    format!("Rewriting the quotation marks: {done} of {total} scenes.")
                }
                Restyled::Finished => format!("Every scene now quotes {}.", to.shown()),
            }),
        html::progress()
            .class("restyling")
            .attr("aria-label", "Rewriting the quotation marks")
            .attr("max", move || shown_on_the_bar(restyled.get()).1)
            .attr("value", move || shown_on_the_bar(restyled.get()).0),
        html::div().class("dialog-actions").child(
            html::button()
                .r#type("button")
                .class("primary")
                .on(ev::click, move |_| changing.set(false))
                .child("Close"),
        ),
    )
}

fn shown_on_the_bar(restyled: Restyled) -> (Option<String>, String) {
    match restyled {
        Restyled::Waiting => (None, "1".to_owned()),
        Restyled::Running { done, total } => (Some(done.to_string()), total.max(1).to_string()),
        Restyled::Finished => (Some("1".to_owned()), "1".to_owned()),
    }
}

fn follow(project: ProjectId, saved_against: Version, restyled: RwSignal<Restyled>) {
    spawn_local(async move {
        let found = scenes::quotation_restyling(&project).await;
        if restyled.is_disposed() {
            return;
        }
        if let Ok(found) = found {
            restyled.set(standing(found, saved_against));
        }
        if restyled.get_untracked() != Restyled::Finished {
            _ = set_timeout(
                move || follow(project, saved_against, restyled),
                FOLLOW_EVERY,
            );
        }
    });
}
