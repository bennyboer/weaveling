use leptos::html;
use leptos::html::Input;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::http::ApiError;
use crate::pieces::model::Piece;
use crate::pieces::service;
use crate::projects::model::ProjectId;

#[component]
pub fn Pool(project: ProjectId) -> impl IntoView {
    let refetch = Trigger::new();
    let problem = RwSignal::new(None::<ApiError>);
    let title: NodeRef<Input> = NodeRef::new();

    let listed = {
        let project = project.clone();

        LocalResource::new(move || {
            refetch.track();
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
                    Ok(_) => {
                        problem.set(None);
                        refetch.notify();
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

    let pieces = move || match listed.get() {
        Some(found) => found.as_ref().cloned().unwrap_or_default(),
        None => Vec::new(),
    };

    html::section().class("pool").child((
        html::h2().child("Pieces"),
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
        move || (pieces().is_empty()).then(|| html::p().class("empty").child(nothing_yet(listed))),
        html::ul()
            .class("pieces")
            .attr("aria-label", "Pieces")
            .child(move || pieces().into_iter().map(row).collect_view()),
    ))
}

fn nothing_yet(listed: LocalResource<Result<Vec<Piece>, ApiError>>) -> &'static str {
    if listed.get().is_none() {
        "Loading…"
    } else {
        "No pieces yet. Shoot an idea in and see where it goes."
    }
}

fn row(piece: Piece) -> impl IntoView {
    html::li().child((
        html::span()
            .class("name")
            .child(piece.shown_as().to_owned()),
        piece
            .passage
            .is_some()
            .then(|| html::span().class("stamp").child("has prose")),
    ))
}
