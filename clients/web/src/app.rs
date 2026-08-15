use leptos::ev;
use leptos::prelude::*;

use crate::confirm_delete::ConfirmDelete;
use crate::new_project::NewProject;
use crate::overlays::Overlays;
use crate::project_row::ProjectRow;
use crate::workspace::Workspace;

#[component]
pub fn App() -> impl IntoView {
    let workspace = Workspace::new();
    let overlays = Overlays::new();

    Effect::new(move |_| workspace.reload());

    window_event_listener(ev::click, move |_| overlays.close_menu());
    window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            overlays.close_all();
        }
    });

    view! {
        <main>
            <h1>"Weaveling"</h1>
            <p class="tagline">
                "Bring us your tiny, fragile story ideas, and we will help you weave them into a full epic."
            </p>

            <NewProject workspace=workspace />

            {move || {
                workspace
                    .problem()
                    .get()
                    .map(|failure| {
                        view! { <p class="problem" role="alert">{failure.to_string()}</p> }
                    })
            }}

            {move || placeholder(workspace)}

            <ul class="projects">
                {move || {
                    workspace
                        .projects()
                        .get()
                        .into_iter()
                        .map(|project| {
                            view! {
                                <ProjectRow
                                    project=project
                                    workspace=workspace
                                    overlays=overlays
                                />
                            }
                        })
                        .collect_view()
                }}
            </ul>

            <ConfirmDelete workspace=workspace overlays=overlays />
        </main>
    }
}

fn placeholder(workspace: Workspace) -> Option<impl IntoView> {
    if !workspace.projects().with(|projects| projects.is_empty()) {
        return None;
    }

    let message = if workspace.busy().get() {
        "Loading…"
    } else {
        "No projects yet. Every epic starts as a little mess."
    };

    Some(view! { <p class="empty">{message}</p> })
}
