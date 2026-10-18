use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::dialog::modal;
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;

#[component]
pub fn ConfirmDelete(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    move || {
        overlays.confirming().get().map(|project| {
            let id = project.id.clone();
            let fate = format!("“{}” will be gone for good.", project.name);

            modal(
                "Delete this project?",
                "",
                move || overlays.dismiss(),
                (
                    html::p().child((fate, " Deleting cannot be undone.")),
                    html::div().class("dialog-actions").child((
                        html::button()
                            .on(ev::click, move |_| overlays.dismiss())
                            .child("Cancel"),
                        html::button()
                            .class("danger")
                            .disabled(workspace.deleting())
                            .on(ev::click, move |_| {
                                overlays.dismiss();
                                workspace.delete(id.clone());
                            })
                            .child("Delete"),
                    )),
                ),
            )
        })
    }
}
