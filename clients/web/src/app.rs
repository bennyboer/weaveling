use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::hooks::use_params_map;
use leptos_router::path;

use crate::http::ApiError;
use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::passages::service as passages;
use crate::pieces::model::PieceId;
use crate::pieces::pool::{Pool, PoolProps};
use crate::pieces::service as pieces;
use crate::projects::confirm_delete::{ConfirmDelete, ConfirmDeleteProps};
use crate::projects::new_project::{NewProject, NewProjectProps};
use crate::projects::overlays::Overlays;
use crate::projects::row::{ProjectRow, ProjectRowProps};
use crate::projects::workspace::Workspace;
use crate::route;

#[component]
pub fn App() -> impl IntoView {
    let workspace = Workspace::new();
    let overlays = Overlays::new();

    window_event_listener(ev::click, move |_| overlays.close_menu());
    window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            overlays.close_all();
        }
    });

    html::main().child((
        html::h1().child("Weaveling"),
        html::p()
            .class("tagline")
            .child(
                "Bring us your tiny, fragile story ideas, and we will help you weave them into a full epic.",
            ),
        view! {
            <Router>
                <Routes fallback=Missing>
                    <Route
                        path=path!("/")
                        view=move || TheWorkspace(TheWorkspaceProps { workspace, overlays })
                    />
                    <Route path=path!("/projects/:project") view=OneProject />
                    <Route path=path!("/projects/:project/pieces/:piece") view=OnePiece />
                </Routes>
            </Router>
        },
    ))
}

#[component]
fn TheWorkspace(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    (
        NewProject(NewProjectProps { workspace }),
        move || {
            workspace.problem().map(|failure| {
                html::p()
                    .class("problem")
                    .role("alert")
                    .child(failure.to_string())
            })
        },
        move || placeholder(workspace),
        html::ul().class("projects").child(move || {
            workspace
                .projects()
                .into_iter()
                .map(|project| {
                    ProjectRow(ProjectRowProps {
                        project,
                        workspace,
                        overlays,
                    })
                })
                .collect_view()
        }),
        ConfirmDelete(ConfirmDeleteProps {
            workspace,
            overlays,
        }),
    )
}

#[component]
fn OneProject() -> impl IntoView {
    let params = use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                view! {
                    <A href=route::WORKSPACE attr:class="back">
                        "All projects"
                    </A>
                },
                Pool(PoolProps {
                    project: route::project_id(&project),
                }),
            )
        })
    }
}

#[component]
fn Missing() -> impl IntoView {
    (
        html::p()
            .class("empty")
            .child("There is nothing woven at this address."),
        view! {
            <A href=route::WORKSPACE attr:class="back">
                "All projects"
            </A>
        },
    )
}

#[component]
fn OnePiece() -> impl IntoView {
    let params = use_params_map();
    let problem = RwSignal::new(None::<ApiError>);
    let passage = RwSignal::new(None::<PassageId>);

    let opening = Action::new_local(move |piece: &PieceId| {
        let piece = piece.clone();

        async move {
            match writing_in(&piece).await {
                Ok(found) => {
                    problem.set(None);
                    passage.set(Some(found));
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    Effect::new(move || {
        if let Some(piece) = params.read().get("piece") {
            opening.dispatch(route::piece_id(&piece));
        }
    });

    let pool = move || {
        params
            .read()
            .get("project")
            .map(|project| format!("/projects/{project}"))
            .unwrap_or_else(|| route::WORKSPACE.to_owned())
    };

    (
        view! {
            <A href=pool attr:class="back">
                "Back to the pool"
            </A>
        },
        move || {
            problem.get().map(|failure| {
                html::p()
                    .class("problem")
                    .role("alert")
                    .child(failure.to_string())
            })
        },
        move || match passage.get() {
            Some(passage) => PassageEditor(PassageEditorProps { passage }).into_any(),
            None => html::p()
                .class("empty")
                .child("Opening the piece…")
                .into_any(),
        },
    )
}

async fn writing_in(piece: &PieceId) -> Result<PassageId, ApiError> {
    let found = pieces::get(piece).await?;

    match found.passage {
        Some(passage) => Ok(passage),
        None => {
            let started = passages::create().await?;

            pieces::attach_passage(piece, &started)
                .await?
                .passage
                .ok_or(ApiError::Unexpected)
        }
    }
}

fn placeholder(workspace: Workspace) -> Option<impl IntoView> {
    if !workspace.projects().is_empty() {
        return None;
    }

    let message = if workspace.loading() {
        "Loading…"
    } else {
        "No projects yet. Every epic starts as a little mess."
    };

    Some(html::p().class("empty").child(message))
}
