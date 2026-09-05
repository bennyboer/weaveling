mod board;
mod missing;
mod piece;
mod pool;
mod workspace;

use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::app::board::OneBoard;
use crate::app::missing::Missing;
use crate::app::piece::OnePiece;
use crate::app::pool::OnePool;
use crate::app::workspace::{TheWorkspace, TheWorkspaceProps};
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;

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
            <Routes fallback=Missing>
                <Route
                    path=path!("/")
                    view=move || TheWorkspace(TheWorkspaceProps { workspace, overlays })
                />
                <Route path=path!("/projects/:project") view=OneBoard />
                <Route path=path!("/projects/:project/pieces") view=OnePool />
                <Route path=path!("/projects/:project/pieces/:piece") view=OnePiece />
            </Routes>
        </Router>
    }
}
