use leptos::prelude::*;
use leptos_router::{LazyRoute, lazy_route};

use crate::accounts::security::SecurityPage;
use crate::app::board::BoardPage;
use crate::app::idea::IdeaPage;
use crate::app::ideas::IdeasPage;
use crate::app::members::{MembersInvitedPage, MembersPage};
use crate::app::outline::OutlinePage;
use crate::app::write::{LooseScenesPage, ProjectPage, WritePage};
use crate::instance::{InstanceAccountsPage, InstanceInvitationsPage, InstancePage};

/// A project, opened at its first view.
pub struct ProjectRoute;

#[lazy_route]
impl LazyRoute for ProjectRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        ProjectPage().into_any()
    }
}

/// The writing view.
/// It is shown only after the editor is fetched too.
/// Fetching the editor starts at the same time as fetching the view's own code.
pub struct WriteRoute {
    editor: LocalResource<()>,
}

#[lazy_route]
impl LazyRoute for WriteRoute {
    fn data() -> Self {
        Self {
            editor: with_the_editor(),
        }
    }

    fn view(this: Self) -> AnyView {
        (move || this.editor.get().map(|()| WritePage())).into_any()
    }
}

/// The scenes that are not in the book.
/// They are shown only after the editor is fetched too.
pub struct LooseScenesRoute {
    editor: LocalResource<()>,
}

#[lazy_route]
impl LazyRoute for LooseScenesRoute {
    fn data() -> Self {
        Self {
            editor: with_the_editor(),
        }
    }

    fn view(this: Self) -> AnyView {
        (move || this.editor.get().map(|()| LooseScenesPage())).into_any()
    }
}

fn with_the_editor() -> LocalResource<()> {
    LocalResource::new(crate::scenes::prose::editor_loaded)
}

/// The board.
pub struct BoardRoute;

#[lazy_route]
impl LazyRoute for BoardRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        BoardPage().into_any()
    }
}

/// The outline.
pub struct OutlineRoute;

#[lazy_route]
impl LazyRoute for OutlineRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        OutlinePage().into_any()
    }
}

/// The ideas.
pub struct IdeasRoute;

#[lazy_route]
impl LazyRoute for IdeasRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        IdeasPage().into_any()
    }
}

/// One idea.
pub struct IdeaRoute;

#[lazy_route]
impl LazyRoute for IdeaRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        IdeaPage().into_any()
    }
}

/// A project's members.
pub struct MembersRoute;

#[lazy_route]
impl LazyRoute for MembersRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        MembersPage().into_any()
    }
}

/// The people invited into a project.
pub struct MembersInvitedRoute;

#[lazy_route]
impl LazyRoute for MembersInvitedRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        MembersInvitedPage().into_any()
    }
}

/// The account's security.
pub struct SecurityRoute;

#[lazy_route]
impl LazyRoute for SecurityRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        SecurityPage().into_any()
    }
}

/// The page for operating this Weaveling instance.
pub struct InstanceRoute;

#[lazy_route]
impl LazyRoute for InstanceRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        InstancePage().into_any()
    }
}

/// The accounts of this Weaveling instance.
pub struct InstanceAccountsRoute;

#[lazy_route]
impl LazyRoute for InstanceAccountsRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        InstanceAccountsPage().into_any()
    }
}

/// The invitations of this Weaveling instance.
pub struct InstanceInvitationsRoute;

#[lazy_route]
impl LazyRoute for InstanceInvitationsRoute {
    fn data() -> Self {
        Self
    }

    fn view(_: Self) -> AnyView {
        InstanceInvitationsPage().into_any()
    }
}
