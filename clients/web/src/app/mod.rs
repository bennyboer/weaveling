mod board;
mod idea;
mod ideas;
/// The views that are loaded only when they are opened.
///
/// The code of each view is in its own chunk, which is fetched the first time the view is opened.
/// So opening Weaveling does not wait for any of them.
/// Because of this, every build is split with `--split`.
mod lazy;
mod members;
mod missing;
mod outline;
mod project_page;
mod trash;
mod workspace;
mod write;

use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{Lazy, path};

use crate::accounts::deletion_pending::DeletionPendingPage;
use crate::accounts::invitation::InvitationPage;
use crate::accounts::sign_in::SignInPage;
use crate::accounts::unreachable::UnreachablePage;
use crate::accounts::your_account::YourAccountPage;
use crate::accounts::{CurrentSession, Screen, SignInState};
use crate::app::lazy::{
    BoardRoute, IdeaRoute, IdeasRoute, InstanceAccountsRoute, InstanceInvitationsRoute,
    InstanceRoute, LooseScenesRoute, MembersInvitedRoute, MembersRoute, OutlineRoute, ProjectRoute,
    SecurityRoute, WriteRoute,
};
use crate::app::missing::MissingPage;
use crate::app::trash::{TrashPage, TrashPageProps};
use crate::app::workspace::{WorkspacePage, WorkspacePageProps};
use crate::members::roles::Roles;
use crate::projects::open_project::OpenProject;
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;
use crate::quiet_note;
use crate::quiet_note::QuietNote;
use crate::setup::SetUpPage;
use crate::shell;
use crate::speech::Speech;

#[component]
pub fn App() -> impl IntoView {
    let overlays = Overlays::none();
    let session = CurrentSession::asked();
    let screen = session.screen();
    provide_context(OpenProject::none());
    provide_context(Speech::heard());
    provide_context(QuietNote::none());
    provide_context(session);
    provide_context(Roles::followed());

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
                Screen::DeletionPending => view! { <DeletionPendingPage /> }.into_any(),
                Screen::Work => work(overlays, session).into_any(),
            }}
            {shell::status_bar()}
            {quiet_note::quiet_note()}
            {crate::accounts::recovery_note()}
        </Router>
    }
}

fn work(overlays: Overlays, session: CurrentSession) -> impl IntoView {
    let with_accounts = untrack(|| matches!(session.state(), SignInState::SignedIn(_)));
    let workspace = Workspace::listing(with_accounts);

    view! {
        <Routes fallback=MissingPage>
            <Route
                path=path!("/")
                view=move || WorkspacePage(WorkspacePageProps { workspace, overlays })
            />
            <Route path=path!("/trash") view=move || TrashPage(TrashPageProps { workspace }) />
            <Route path=path!("/invitation") view=InvitationPage />
            <Route path=path!("/setup") view=SetUpPage />
            <Route path=path!("/account") view=YourAccountPage />
            <Route path=path!("/account/security") view={Lazy::<SecurityRoute>::new()} />
            <Route path=path!("/instance") view={Lazy::<InstanceRoute>::new()} />
            <Route path=path!("/instance/accounts") view={Lazy::<InstanceAccountsRoute>::new()} />
            <Route path=path!("/instance/invitations") view={Lazy::<InstanceInvitationsRoute>::new()} />
            <Route path=path!("/projects/:project") view={Lazy::<ProjectRoute>::new()} />
            <Route path=path!("/projects/:project/board") view={Lazy::<BoardRoute>::new()} />
            <Route path=path!("/projects/:project/outline") view={Lazy::<OutlineRoute>::new()} />
            <Route path=path!("/projects/:project/ideas") view={Lazy::<IdeasRoute>::new()} />
            <Route path=path!("/projects/:project/members") view={Lazy::<MembersRoute>::new()} />
            <Route path=path!("/projects/:project/members/invited") view={Lazy::<MembersInvitedRoute>::new()} />
            <Route path=path!("/projects/:project/write") view={Lazy::<WriteRoute>::new()} />
            <Route path=path!("/projects/:project/write/loose") view={Lazy::<LooseScenesRoute>::new()} />
            <Route path=path!("/projects/:project/ideas/:idea") view={Lazy::<IdeaRoute>::new()} />
        </Routes>
    }
}
