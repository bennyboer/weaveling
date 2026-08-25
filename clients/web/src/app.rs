use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::passages::service as passages;
use crate::projects::confirm_delete::{ConfirmDelete, ConfirmDeleteProps};
use crate::projects::new_project::{NewProject, NewProjectProps};
use crate::projects::overlays::Overlays;
use crate::projects::row::{ProjectRow, ProjectRowProps};
use crate::projects::workspace::Workspace;

#[component]
pub fn App() -> impl IntoView {
    let workspace = Workspace::new();
    let overlays = Overlays::new();
    let open_passage = RwSignal::new(None::<PassageId>);
    let starting = Action::new_local(move |()| async move {
        if let Ok(started) = passages::create().await {
            open_passage.set(Some(started));
        }
    });

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
        move || match open_passage.get() {
            Some(passage) => PassageEditor(PassageEditorProps { passage }).into_any(),
            None => html::button()
                .disabled(move || starting.pending().get())
                .on(ev::click, move |_| {
                    starting.dispatch(());
                })
                .child("Start writing")
                .into_any(),
        },
    ))
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
