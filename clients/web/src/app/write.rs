use leptos::IntoView;
use leptos::prelude::*;
use leptos::view;
use leptos_router::NavigateOptions;
use leptos_router::components::Redirect;
use leptos_router::hooks;

use crate::app::project_page;
use crate::members::roles::Roles;
use crate::route;
use crate::scenes::model::SceneId;
use crate::shell::Viewing;
use crate::writing::side::Side;
use crate::writing::write_view::{WriteView, WriteViewProps};

#[component]
pub fn ProjectPage() -> impl IntoView {
    let params = hooks::use_params_map();

    move || {
        params
            .read()
            .get("project")
            .map(|project| view! { <Redirect path=route::writing(&project) options=replacing() /> })
    }
}

fn replacing() -> NavigateOptions {
    NavigateOptions {
        replace: true,
        ..Default::default()
    }
}

#[component]
pub fn WritePage() -> impl IntoView {
    writing_on(Side::Book)
}

#[component]
pub fn LooseScenesPage() -> impl IntoView {
    writing_on(Side::Loose)
}

fn writing_on(side: Side) -> impl IntoView {
    let following = untrack(|| hooks::use_query_map().read().get("scene")).map(SceneId::from);

    let roles = expect_context::<Roles>();

    project_page::project_page(Viewing::Write, move |project| {
        let may_write = {
            let project = project.clone();

            Memo::new(move |_| roles.may_write(&project))
        };
        let following = following.clone();

        move || match may_write.get() {
            true => WriteView(WriteViewProps {
                project: project.clone(),
                following: following.clone(),
                side,
            })
            .into_any(),
            false => {
                view! { <Redirect path=route::board(&project) options=replacing() /> }.into_any()
            }
        }
    })
}
