use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};

use crate::problem::problem;
use crate::projects::confirm_delete::{ConfirmDelete, ConfirmDeleteProps};
use crate::projects::new_project::{NewProject, NewProjectProps};
use crate::projects::overlays::Overlays;
use crate::projects::row::{ProjectRow, ProjectRowProps};
use crate::projects::workspace::Workspace;
use crate::shell::masthead;
use crate::speech::said;

#[component]
pub fn WorkspacePage(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    (
        masthead(None),
        html::main().child(html::div().class("column").child((
            html::h1().child(said("workspace-heading")),
            html::p().class("tagline").child(said("workspace-tagline")),
            NewProject(NewProjectProps { workspace }),
            move || workspace.problem().map(problem),
            move || placeholder(workspace),
            html::ul().class("projects").child(view! {
                <For
                    each=move || workspace.projects()
                    key=|project| (project.id.clone(), project.version)
                    let:project
                >
                    {ProjectRow(ProjectRowProps {
                        project,
                        workspace,
                        overlays,
                    })}
                </For>
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
        "loading"
    } else {
        "workspace-empty"
    };

    Some(html::p().class("empty").child(said(message)))
}
