use fluent_bundle::FluentArgs;
use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::dialog;
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;
use crate::speech;
use crate::speech::Speech;

#[component]
pub fn ConfirmTrash(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    let speech = expect_context::<Speech>();

    move || {
        overlays.confirming().get().map(|project| {
            let id = project.id.clone();
            let name = project.name.clone();
            let fate = move || {
                let mut args = FluentArgs::new();
                args.set("name", name.clone());

                speech.say_with("trash-project-fate", &args)
            };

            dialog::modal(
                Signal::derive(speech::said("trash-project-dialog")),
                "",
                move || overlays.dismiss(),
                (
                    html::p().child(fate),
                    html::div().class("dialog-actions").child((
                        html::button()
                            .on(ev::click, move |_| overlays.dismiss())
                            .child(speech::said("trash-project-cancel")),
                        html::button()
                            .class("danger")
                            .attr("aria-busy", move || workspace.trashing())
                            .on(ev::click, move |_| {
                                overlays.dismiss();
                                workspace.trash(id.clone());
                            })
                            .child(speech::said("trash-project-confirm")),
                    )),
                ),
            )
        })
    }
}
