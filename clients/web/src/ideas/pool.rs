use leptos::html;
use leptos::html::Input;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::http::ApiError;
use crate::ideas::model::Idea;
use crate::ideas::service;
use crate::route;

#[component]
pub fn Pool(project: String) -> impl IntoView {
    let whose = project;
    let project = route::project_id(&whose);
    let problem = RwSignal::new(None::<ApiError>);
    let captured = RwSignal::new(Vec::<Idea>::new());
    let title: NodeRef<Input> = NodeRef::new();

    let listed = {
        let project = project.clone();

        LocalResource::new(move || {
            let project = project.clone();

            async move { service::list(&project).await }
        })
    };

    let capturing = {
        let project = project.clone();

        Action::new_local(move |saying: &String| {
            let project = project.clone();
            let saying = saying.clone();

            async move {
                match service::capture(&project, &saying).await {
                    Ok(idea) => {
                        problem.set(None);
                        captured.update(|held| held.push(idea));
                    }
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        })
    };

    let capture = move || {
        let field = title.get().expect("the title field should be mounted");
        capturing.dispatch(field.value());
        field.set_value("");
    };

    let ideas = move || {
        let mut shown = match listed.get() {
            Some(found) => found.as_ref().cloned().unwrap_or_default(),
            None => Vec::new(),
        };

        for idea in captured.get() {
            if !shown.iter().any(|already| already.id == idea.id) {
                shown.push(idea);
            }
        }

        shown
    };

    html::section().class("pool").child((
        html::h2().child("Ideas"),
        html::form()
            .class("capture")
            .on(ev::submit, move |event| {
                event.prevent_default();
                capture();
            })
            .child((
                html::input()
                    .r#type("text")
                    .attr("aria-label", "What is the idea?")
                    .placeholder("What is the idea?")
                    .node_ref(title),
                html::button()
                    .r#type("submit")
                    .disabled(capturing.pending())
                    .child("Capture"),
            )),
        move || {
            problem.get().map(|failure| {
                html::p()
                    .class("problem")
                    .role("alert")
                    .child(failure.to_string())
            })
        },
        move || (ideas().is_empty()).then(|| html::p().class("empty").child(nothing_yet(listed))),
        html::ul()
            .class("ideas")
            .attr("aria-label", "Ideas")
            .child(move || {
                ideas()
                    .into_iter()
                    .map(|idea| row(whose.clone(), idea))
                    .collect_view()
            }),
    ))
}

fn nothing_yet(listed: LocalResource<Result<Vec<Idea>, ApiError>>) -> &'static str {
    if listed.get().is_none() {
        "Loading…"
    } else {
        "No ideas yet. Shoot an idea in and see where it goes."
    }
}

fn row(project: String, idea: Idea) -> impl IntoView {
    let href = route::idea(&project, &idea.id, &idea.title);
    let shown = idea.shown_as().to_owned();

    html::li().child((
        view! {
            <A href=href attr:class="name">
                {shown}
            </A>
        },
        idea.passage.is_some().then(opened_for_writing),
    ))
}

fn opened_for_writing() -> impl IntoView {
    let quill = view! {
        <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
            <path
                d="M11.5 1.5 14.5 4.5 5.5 13.5 1.5 14.5 2.5 10.5z"
                fill="none"
                stroke="currentColor"
                stroke-width="1.3"
                stroke-linejoin="round"
            />
        </svg>
    };

    html::span()
        .class("stamp")
        .role("img")
        .attr("aria-label", "Opened for writing")
        .child(quill)
}
