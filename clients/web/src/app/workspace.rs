use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;

use crate::projects::confirm_delete::{ConfirmDelete, ConfirmDeleteProps};
use crate::projects::new_project::{NewProject, NewProjectProps};
use crate::projects::overlays::Overlays;
use crate::projects::row::{ProjectRow, ProjectRowProps};
use crate::projects::workspace::Workspace;
use crate::shell::masthead;

#[component]
pub fn TheWorkspace(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    (
        masthead(None),
        html::main().child(html::div().class("column").child((
            html::h1().child("Your projects"),
            html::p().class("tagline").child(
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
        ))),
    )
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
