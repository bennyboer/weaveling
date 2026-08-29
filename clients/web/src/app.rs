use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::passages::service as passages;
use crate::pieces::pool::{Pool, PoolProps};
use crate::projects::confirm_delete::{ConfirmDelete, ConfirmDeleteProps};
use crate::projects::model::ProjectId;
use crate::projects::new_project::{NewProject, NewProjectProps};
use crate::projects::overlays::Overlays;
use crate::projects::row::{ProjectRow, ProjectRowProps};
use crate::projects::workspace::Workspace;
use crate::route::{self, Route};
use crate::url;

const PASSAGE: &str = "passage";

#[component]
pub fn App() -> impl IntoView {
    let workspace = Workspace::new();
    let overlays = Overlays::new();
    let here = RwSignal::new(route::current());

    let travel = Callback::new(move |route: Route| {
        route::go(&route);
        here.set(route);
    });

    window_event_listener(ev::popstate, move |_| here.set(route::current()));
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
        move || match here.get() {
            Route::Workspace => TheWorkspace(TheWorkspaceProps {
                workspace,
                overlays,
                travel,
            })
            .into_any(),
            Route::Project(project) => {
                OneProject(OneProjectProps { project, travel }).into_any()
            }
        },
    ))
}

#[component]
fn TheWorkspace(
    workspace: Workspace,
    overlays: Overlays,
    travel: Callback<Route>,
) -> impl IntoView {
    let open = Callback::new(move |project: ProjectId| travel.run(Route::Project(project)));

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
                        on_open: open,
                    })
                })
                .collect_view()
        }),
        ConfirmDelete(ConfirmDeleteProps {
            workspace,
            overlays,
        }),
        TheLegacyEditor(),
    )
}

#[component]
fn OneProject(project: ProjectId, travel: Callback<Route>) -> impl IntoView {
    (
        html::button()
            .class("back")
            .on(ev::click, move |_| travel.run(Route::Workspace))
            .child("All projects"),
        Pool(PoolProps { project }),
    )
}

#[component]
fn TheLegacyEditor() -> impl IntoView {
    let open_passage = RwSignal::new(None::<PassageId>);

    let starting = Action::new_local(move |()| async move {
        if let Ok(started) = passages::create().await {
            url::remember(PASSAGE, started.as_str());
            open_passage.set(Some(started));
        }
    });
    let reopening = Action::new_local(move |remembered: &PassageId| {
        let remembered = remembered.clone();
        async move {
            if passages::confirm(&remembered).await.is_ok() {
                open_passage.set(Some(remembered));
            } else {
                url::forget(PASSAGE);
            }
        }
    });

    if let Some(remembered) = url::query(PASSAGE) {
        reopening.dispatch(PassageId::from(remembered));
    }

    move || match (open_passage.get(), reopening.pending().get()) {
        (Some(passage), _) => (
            PassageEditor(PassageEditorProps { passage }),
            html::button()
                .on(ev::click, move |_| {
                    url::forget(PASSAGE);
                    open_passage.set(None);
                })
                .child("Stop writing"),
        )
            .into_any(),
        (None, true) => html::p()
            .class("empty")
            .child("Reopening your passage…")
            .into_any(),
        (None, false) => html::button()
            .disabled(starting.pending())
            .on(ev::click, move |_| {
                starting.dispatch(());
            })
            .child("Start writing")
            .into_any(),
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
