use fluent_bundle::FluentArgs;
use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev};
use quotation::QuotationMarks;
use scenes_contract::QuotationRestylingDTO;

use crate::act::{Act, Here};
use crate::dialog;
use crate::http::ApiError;
use crate::icons;
use crate::icons::Icon;
use crate::live::Subscription;
use crate::members::roles::Roles;
use crate::problem;
use crate::projects::details_form;
use crate::projects::details_form::DetailsDraft;
use crate::projects::intent::Intent;
use crate::projects::model::{DescribedProject, ProjectId, Version};
use crate::projects::open_project::OpenProject;
use crate::scenes::restyling::Restyled;
use crate::scenes::service as scenes;
use crate::speech;
use crate::speech::Speech;
use live_contract::FrameDTO;

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
            .attr("aria-label", speech::said("project-details"))
            .attr("title", speech::said("project-details"))
            .disabled(move || open_project.described().is_none())
            .on(ev::click, move |_| changing.set(true))
            .child(icons::mark(Icon::Cogwheel)),
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
    let roles = expect_context::<Roles>();
    let owning = {
        let project = project.to_string();

        Memo::new(move |_| roles.owns(&project))
    };
    let speech = expect_context::<Speech>();
    let draft = DetailsDraft::of(RwSignal::new(known.name.clone()), &known.details);
    let field: NodeRef<Input> = NodeRef::new();
    let refused = RwSignal::new(None::<ApiError>);
    let stage = RwSignal::new(Stage::Editing);
    let restyled = RwSignal::new(Restyled::Waiting);
    let following = StoredValue::new_local(None::<Subscription>);
    let project = StoredValue::new(project);
    let known = StoredValue::new(known);

    let saving = Act::new(move |(), _: Here| {
        let name = draft.title.get_untracked().trim().to_owned();
        let details = draft.details();
        let project = project.get_value();
        let before = known.get_value();
        let (from, to) = (before.details.quotation_marks(), details.quotation_marks());

        async move {
            let renamed = match name == before.name {
                true => Ok(()),
                false => {
                    open_project
                        .intended(&project, Intent::Rename { name })
                        .await
                }
            };
            let changed = match renamed {
                Ok(()) => {
                    open_project
                        .intended(
                            &project,
                            Intent::ChangeDetails {
                                details,
                                seen: before.details,
                            },
                        )
                        .await
                }
                Err(failure) => Err(failure),
            };

            if let Err(failure) = changed {
                refused.set(Some(failure));
                stage.set(Stage::Editing);
            } else if from == to {
                changing.set(false);
            } else {
                let saved_against = open_project.details_built_on().unwrap_or(before.version);
                stage.set(Stage::Restyling { to });
                following.set_value(Some(follow(project, saved_against, restyled)));
            }
        }
    });

    let save = move |confirmed: bool| {
        if draft.title.get_untracked().trim().is_empty() {
            return;
        }
        let from = known.with_value(|before| before.details.quotation_marks());
        let to = draft.details().quotation_marks();
        if from != to && !confirmed {
            stage.set(Stage::Confirming { from, to });
            return;
        }
        saving.start(());
    };

    dialog::modal(
        Signal::derive(speech::said("project-details")),
        "project-details",
        move || changing.set(false),
        move || match stage.get() {
            Stage::Editing => (
                html::fieldset()
                    .class("details-fields")
                    .prop("disabled", move || !owning.get())
                    .child(details_form::details_fields(
                        draft,
                        field,
                        move || refused.set(None),
                        move || save(false),
                    )),
                move || refused.get().map(problem::problem),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| changing.set(false))
                        .child(speech::said("project-details-cancel")),
                    move || {
                        owning.get().then(|| {
                            html::button()
                                .r#type("button")
                                .class("primary")
                                .disabled(move || draft.title.get().trim().is_empty())
                                .attr("aria-busy", move || saving.busy())
                                .on(ev::click, move |_| save(false))
                                .child(speech::said("project-details-save"))
                        })
                    },
                )),
            )
                .into_any(),
            Stage::Confirming { from, to } => (
                html::p().child(move || {
                    let mut args = FluentArgs::new();
                    args.set("from", from.shown());
                    args.set("to", to.shown());

                    speech.say_with("project-details-changing-the-marks", &args)
                }),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| stage.set(Stage::Editing))
                        .child(speech::said("project-details-back")),
                    html::button()
                        .r#type("button")
                        .class("primary")
                        .attr("aria-busy", move || saving.busy())
                        .on(ev::click, move |_| save(true))
                        .child(speech::said("project-details-change-the-marks")),
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
    let speech = expect_context::<Speech>();

    (
        html::p().attr("aria-live", "polite").child(move || {
            let mut args = FluentArgs::new();
            match restyled.get() {
                Restyled::Waiting => speech.say("restyling-waiting"),
                Restyled::Running { done, total } => {
                    args.set("done", done);
                    args.set("total", total);

                    speech.say_with("restyling-running", &args)
                }
                Restyled::Finished => {
                    args.set("marks", to.shown());

                    speech.say_with("restyling-finished", &args)
                }
            }
        }),
        html::progress()
            .class("restyling")
            .attr("aria-label", speech::said("restyling"))
            .attr("max", move || shown_on_the_bar(restyled.get()).1)
            .attr("value", move || shown_on_the_bar(restyled.get()).0),
        html::div().class("dialog-actions").child(
            html::button()
                .r#type("button")
                .class("primary")
                .on(ev::click, move |_| changing.set(false))
                .child(speech::said("restyling-close")),
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

fn follow(
    project: ProjectId,
    saved_against: Version,
    restyled: RwSignal<Restyled>,
) -> Subscription {
    let asked = project.clone();

    Subscription::start(
        "quotation-restylings",
        &[("project", project.to_string().as_str())],
        move |told: FrameDTO| {
            if let Ok(found) = serde_json::from_value::<QuotationRestylingDTO>(told.event) {
                let found = crate::scenes::restyling::standing(Some(found.into()), saved_against);
                _ = restyled.try_update(|held| *held = held.advanced_to(found));
            }
        },
        move || {
            let project = asked.clone();
            task::spawn_local(async move {
                if let Ok(found) = scenes::quotation_restyling(&project).await {
                    let found = crate::scenes::restyling::standing(found, saved_against);
                    _ = restyled.try_update(|held| *held = held.advanced_to(found));
                }
            });
        },
    )
}
