use leptos::prelude::*;

use crate::overlays::Overlays;
use crate::workspace::Workspace;

#[component]
pub fn ConfirmDelete(workspace: Workspace, overlays: Overlays) -> impl IntoView {
    move || {
        overlays.confirming().get().map(|project| {
            let id = project.id.clone();

            view! {
                <div class="overlay" on:click=move |_| overlays.dismiss()>
                    <div
                        class="dialog"
                        role="dialog"
                        aria-modal="true"
                        on:click=move |event| event.stop_propagation()
                    >
                        <h2>"Delete this project?"</h2>
                        <p>
                            {format!("“{}” will be gone for good.", project.name)}
                            " Deleting cannot be undone."
                        </p>
                        <div class="dialog-actions">
                            <button on:click=move |_| overlays.dismiss()>"Cancel"</button>
                            <button
                                class="danger"
                                on:click=move |_| {
                                    overlays.dismiss();
                                    workspace.delete(id.clone());
                                }
                            >
                                "Delete"
                            </button>
                        </div>
                    </div>
                </div>
            }
        })
    }
}
