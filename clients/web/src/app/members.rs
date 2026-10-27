use leptos::prelude::*;
use leptos::{IntoView, component, html};
use leptos_router::hooks;

use crate::members::people;
use crate::route;
use crate::shell;
use crate::shell::Inside;
use crate::speech;

#[component]
pub fn MembersPage() -> impl IntoView {
    let params = hooks::use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                shell::masthead(Some(Inside::of(&project, None))),
                html::main().class("operating members-page").child((
                    html::h1().child(speech::said("members-heading")),
                    html::p().class("hint").child(speech::said("members-hint")),
                    people::people(route::project_id(&project)),
                    html::p().class("hint").child(speech::said("members-roles")),
                )),
            )
        })
    }
}
