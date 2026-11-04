use leptos::html;
use leptos::html::Input;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::ideas::followed;
use crate::ideas::model;
use crate::ideas::model::Idea;
use crate::ideas::service;
use crate::members::roles::Roles;
use crate::problem;
use crate::route;
use crate::speech;

#[component]
pub fn Pool(project: String) -> impl IntoView {
    let whose = project;
    let project = route::project_id(&whose);
    let roles = expect_context::<Roles>();
    let may_write = {
        let whose = whose.clone();

        Memo::new(move |_| roles.may_write(&whose))
    };
    let failed = RwSignal::new(None::<ApiError>);
    let held = RwSignal::new(None::<Vec<Idea>>);
    let title: NodeRef<Input> = NodeRef::new();

    let asking = followed::asking(&project, held, move |failure| {
        _ = failed.try_set(Some(failure));
    });
    StoredValue::new_local(followed::following(&project, held, asking));

    let capturing = {
        let project = project.clone();

        Act::new(move |(), _: Here| {
            let project = project.clone();
            let field = title.get().expect("the title field should be mounted");
            let saying = field.value();
            field.set_value("");

            async move {
                match service::capture(&project, &saying).await {
                    Ok(idea) => {
                        failed.set(None);
                        held.update(|known| {
                            if let Some(known) = known {
                                model::hold(known, idea);
                            }
                        });
                    }
                    Err(failure) => failed.set(Some(failure)),
                }
            }
        })
    };

    let ideas = move || held.get().unwrap_or_default();

    html::section().class("pool").child((
        html::h2().child(speech::said("ideas")),
        move || {
            may_write.get().then(|| {
                html::form()
                    .class("capture")
                    .on(ev::submit, move |event| {
                        event.prevent_default();
                        capturing.start(());
                    })
                    .child((
                        html::input()
                            .r#type("text")
                            .attr("aria-label", speech::said("idea-asked"))
                            .placeholder(speech::said("idea-asked"))
                            .node_ref(title),
                        html::button()
                            .r#type("submit")
                            .attr("aria-busy", move || capturing.busy())
                            .child(speech::said("ideas-capture")),
                    ))
            })
        },
        move || failed.get().map(problem::problem),
        move || {
            (ideas().is_empty()).then(|| {
                html::p()
                    .class("empty")
                    .child(speech::said(nothing_yet(held)))
            })
        },
        html::ul()
            .class("ideas")
            .attr("aria-label", speech::said("ideas"))
            .child(move || {
                ideas()
                    .into_iter()
                    .map(|idea| row(whose.clone(), idea))
                    .collect_view()
            }),
    ))
}

fn nothing_yet(held: RwSignal<Option<Vec<Idea>>>) -> &'static str {
    if held.with(Option::is_none) {
        "loading"
    } else {
        "ideas-empty"
    }
}

fn row(project: String, idea: Idea) -> impl IntoView {
    let href = route::idea(&project, &idea.id, &idea.title);
    let shown = idea.shown_as().to_owned();

    html::li().child((view! {
        <A href=href attr:class="name">
            {shown}
        </A>
    },))
}
