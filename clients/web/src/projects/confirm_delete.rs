use fluent_bundle::FluentArgs;
use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::dialog::modal;
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;
use crate::speech::{Speech, said};

#[component]
pub fn ConfirmDelete(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    let speech = expect_context::<Speech>();

    move || {
        overlays.confirming().get().map(|project| {
            let id = project.id.clone();
            let name = project.name.clone();
            let fate = move || {
                let mut args = FluentArgs::new();
                args.set("name", name.clone());

                speech.say_with("delete-project-fate", &args)
            };

            modal(
                Signal::derive(said("delete-project-dialog")),
                "",
                move || overlays.dismiss(),
                (
                    html::p().child(fate),
                    html::div().class("dialog-actions").child((
                        html::button()
                            .on(ev::click, move |_| overlays.dismiss())
                            .child(said("delete-project-cancel")),
                        html::button()
                            .class("danger")
                            .disabled(workspace.deleting())
                            .on(ev::click, move |_| {
                                overlays.dismiss();
                                workspace.delete(id.clone());
                            })
                            .child(said("delete-project-confirm")),
                    )),
                ),
            )
        })
    }
}
