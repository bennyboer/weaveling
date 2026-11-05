use fluent_bundle::FluentArgs;
use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos_router::components::A;

use crate::icons;
use crate::icons::Icon;
use crate::problem;
use crate::projects::confirm_trash::{ConfirmTrash, ConfirmTrashProps};
use crate::projects::new_project::{NewProject, NewProjectProps};
use crate::projects::overlays::Overlays;
use crate::projects::row::{ProjectRow, ProjectRowProps};
use crate::projects::workspace::Workspace;
use crate::route;
use crate::shell;
use crate::speech;
use crate::speech::Speech;

#[component]
pub fn WorkspacePage(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    (
        shell::masthead(None),
        html::main().child(
            html::div().class("column").child((
                html::div().class("workspace-heading").child((
                    html::h1().child(speech::said("workspace-heading")),
                    move || trash_link(workspace),
                )),
                html::p()
                    .class("tagline")
                    .child(speech::said("workspace-tagline")),
                NewProject(NewProjectProps { workspace }),
                move || workspace.problem().map(problem::problem),
                move || placeholder(workspace),
                html::ul().class("projects").child(view! {
                    <For
                        each=move || workspace.projects()
                        key=|project| (project.id.clone(), project.version, project.name.clone())
                        let:project
                    >
                        {ProjectRow(ProjectRowProps {
                            project,
                            workspace,
                            overlays,
                        })}
                    </For>
                }),
                ConfirmTrash(ConfirmTrashProps {
                    workspace,
                    overlays,
                }),
            )),
        ),
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

    Some(html::p().class("empty").child(speech::said(message)))
}

fn trash_link(workspace: Workspace) -> Option<impl IntoView> {
    let held = workspace.in_the_trash().len();
    if held == 0 {
        return None;
    }
    let speech = expect_context::<Speech>();
    let told = move || {
        let mut args = FluentArgs::new();
        args.set("count", held);

        speech.say_with("workspace-trash-link", &args)
    };

    Some(view! {
        <A href=route::TRASH attr:class="trash-link" attr:aria-label=told attr:title=told>
            {icons::mark(Icon::Trash)}
            <span class="count" aria-hidden="true">
                {held.to_string()}
            </span>
        </A>
    })
}
