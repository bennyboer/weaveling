use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::scenes::model::SceneId;
use crate::shell::{Inside, Viewing, masthead};
use crate::writing::side::Side;
use crate::writing::write_view::{WriteView, WriteViewProps};

#[component]
pub fn WritePage() -> impl IntoView {
    writing_on(Side::Book)
}

#[component]
pub fn LooseScenesPage() -> impl IntoView {
    writing_on(Side::Loose)
}

fn writing_on(side: Side) -> impl IntoView {
    let params = use_params_map();
    let following = untrack(|| use_query_map().read().get("scene")).map(SceneId::from);

    move || {
        params.read().get("project").map(|project| {
            (
                masthead(Some(Inside::of(&project, Some(Viewing::Write)))),
                html::main().child(WriteView(WriteViewProps {
                    project,
                    following: following.clone(),
                    side,
                })),
            )
        })
    }
}
