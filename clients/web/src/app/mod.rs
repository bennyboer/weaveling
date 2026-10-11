mod board;
mod idea;
mod ideas;
mod missing;
mod outline;
mod scene;
mod workspace;
mod write;

use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::app::board::BoardPage;
use crate::app::idea::IdeaPage;
use crate::app::ideas::IdeasPage;
use crate::app::missing::MissingPage;
use crate::app::outline::OutlinePage;
use crate::app::scene::ScenePage;
use crate::app::workspace::{WorkspacePage, WorkspacePageProps};
use crate::app::write::WritePage;
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;
use crate::shell::status_bar;

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

    view! {
        <Router>
            <Routes fallback=MissingPage>
                <Route
                    path=path!("/")
                    view=move || WorkspacePage(WorkspacePageProps { workspace, overlays })
                />
                <Route path=path!("/projects/:project") view=BoardPage />
                <Route path=path!("/projects/:project/outline") view=OutlinePage />
                <Route path=path!("/projects/:project/ideas") view=IdeasPage />
                <Route path=path!("/projects/:project/write") view=WritePage />
                <Route path=path!("/projects/:project/ideas/:idea") view=IdeaPage />
                <Route
                    path=path!("/projects/:project/scenes/:scene")
                    view=ScenePage
                />
            </Routes>
            {status_bar()}
        </Router>
    }
}
