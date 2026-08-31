use leptos::html;
use leptos::html::Input;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::http::ApiError;
use crate::pieces::model::Piece;
use crate::pieces::service;
use crate::projects::model::ProjectId;
use crate::route;

#[component]
pub fn Pool(project: ProjectId) -> impl IntoView {
    let problem = RwSignal::new(None::<ApiError>);
    let captured = RwSignal::new(Vec::<Piece>::new());
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
                    Ok(piece) => {
                        problem.set(None);
                        captured.update(|held| held.push(piece));
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

    let pieces = move || {
        let mut shown = match listed.get() {
            Some(found) => found.as_ref().cloned().unwrap_or_default(),
            None => Vec::new(),
        };

        for piece in captured.get() {
            if !shown.iter().any(|already| already.id == piece.id) {
                shown.push(piece);
            }
        }

        shown
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
    let href = format!("pieces/{}", route::piece_segment(&piece.id, &piece.title));
    let shown = piece.shown_as().to_owned();

    html::li().child((
        view! {
            <A href=href attr:class="name">
                {shown}
            </A>
        },
        piece.passage.is_some().then(opened_for_writing),
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
