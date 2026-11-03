use fluent_bundle::FluentArgs;
use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::moment;
use crate::problem;
use crate::projects::model::TrashedProject;
use crate::projects::workspace::Workspace;
use crate::route;
use crate::shell;
use crate::speech;
use crate::speech::Speech;

#[component]
pub fn TrashPage(workspace: Workspace) -> impl IntoView {
    (
        shell::masthead(None),
        html::main().child(
            html::div().class("column").child((
                html::h1().child(speech::said("trash-heading")),
                view! {
                    <A href=route::WORKSPACE attr:class="back">
                        {speech::said("trash-back")}
                    </A>
                },
                move || workspace.problem().map(problem::problem),
                move || placeholder(workspace),
                html::ul()
                    .class("projects trash")
                    .attr("aria-label", speech::said("trash-heading"))
                    .child(view! {
                        <For
                            each=move || workspace.in_the_trash()
                            key=|project| project.id.clone()
                            let:project
                        >
                            {trashed_row(project, workspace)}
                        </For>
                    }),
            )),
        ),
    )
}

fn placeholder(workspace: Workspace) -> Option<impl IntoView> {
    if !workspace.in_the_trash().is_empty() {
        return None;
    }

    let message = if workspace.trash_asked() {
        "trash-empty"
    } else {
        "loading"
    };

    Some(html::p().class("empty").child(speech::said(message)))
}

fn trashed_row(project: TrashedProject, workspace: Workspace) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let kept_until = moment::human_time(project.until);
    let until = move || {
        let mut args = FluentArgs::new();
        args.set("moment", kept_until.clone());

        speech.say_with("trash-kept-until", &args)
    };
    let id = StoredValue::new(project.id);

    html::li().child((
        html::span().class("name").child(project.name),
        html::span().class("stamp").child(until),
        html::div().class("actions").child(
            html::button()
                .disabled(move || workspace.restoring().get())
                .on(ev::click, move |_| workspace.restore(id.get_value()))
                .child(speech::said("trash-restore")),
        ),
    ))
}
