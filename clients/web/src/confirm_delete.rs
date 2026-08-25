use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::overlays::Overlays;
use crate::workspace::Workspace;

#[component]
pub fn ConfirmDelete(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    move || {
        overlays.confirming().get().map(|project| {
            let id = project.id.clone();
            let fate = format!("“{}” will be gone for good.", project.name);

            html::div()
                .class("overlay")
                .on(ev::click, move |_| overlays.dismiss())
                .child(
                    html::div()
                        .class("dialog")
                        .role("dialog")
                        .attr("aria-modal", "true")
                        .on(ev::click, |event| event.stop_propagation())
                        .child((
                            html::h2().child("Delete this project?"),
                            html::p().child((fate, " Deleting cannot be undone.")),
                            html::div().class("dialog-actions").child((
                                html::button()
                                    .on(ev::click, move |_| overlays.dismiss())
                                    .child("Cancel"),
                                html::button()
                                    .class("danger")
                                    .disabled(move || workspace.deleting().get())
                                    .on(ev::click, move |_| {
                                        overlays.dismiss();
                                        workspace.delete(id.clone());
                                    })
                                    .child("Delete"),
                            )),
                        )),
                )
        })
    }
}
