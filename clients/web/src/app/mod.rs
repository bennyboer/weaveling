mod board;
mod idea;
mod ideas;
mod missing;
mod outline;
mod project_page;
mod workspace;
mod write;

use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::accounts::sign_in::SignInPage;
use crate::accounts::unreachable::UnreachablePage;
use crate::accounts::{CurrentSession, Screen};
use crate::app::board::BoardPage;
use crate::app::idea::IdeaPage;
use crate::app::ideas::IdeasPage;
use crate::app::missing::MissingPage;
use crate::app::outline::OutlinePage;
use crate::app::workspace::{WorkspacePage, WorkspacePageProps};
use crate::app::write::{LooseScenesPage, ProjectPage, WritePage};
use crate::projects::open_project::OpenProject;
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;
use crate::shell::status_bar;
use crate::speech::Speech;

#[component]
pub fn App() -> impl IntoView {
    let overlays = Overlays::none();
    let session = CurrentSession::asked();
    let screen = session.screen();
    provide_context(OpenProject::none());
    provide_context(Speech::heard());
    provide_context(session);

    window_event_listener(ev::click, move |_| overlays.close_menu());
    window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            overlays.close_all();
        }
    });

    view! {
        <Router>
            {move || match screen.get() {
                Screen::Blank => ().into_any(),
                Screen::Unreachable => view! { <UnreachablePage /> }.into_any(),
                Screen::SignIn => view! { <SignInPage /> }.into_any(),
                Screen::Work => work(overlays).into_any(),
            }}
            {status_bar()}
        </Router>
    }
}

fn work(overlays: Overlays) -> impl IntoView {
    let workspace = Workspace::listing();

    view! {
        <Routes fallback=MissingPage>
            <Route
                path=path!("/")
                view=move || WorkspacePage(WorkspacePageProps { workspace, overlays })
            />
            <Route path=path!("/projects/:project") view=ProjectPage />
            <Route path=path!("/projects/:project/board") view=BoardPage />
            <Route path=path!("/projects/:project/outline") view=OutlinePage />
            <Route path=path!("/projects/:project/ideas") view=IdeasPage />
            <Route path=path!("/projects/:project/write") view=WritePage />
            <Route path=path!("/projects/:project/write/loose") view=LooseScenesPage />
            <Route path=path!("/projects/:project/ideas/:idea") view=IdeaPage />
        </Routes>
    }
}
