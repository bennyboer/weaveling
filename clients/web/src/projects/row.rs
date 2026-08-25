use leptos::html;
use leptos::html::Input;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

use crate::projects::model::Project;
use crate::projects::overlays::Overlays;
use crate::projects::workspace::Workspace;

#[component]
pub fn ProjectRow(project: Project, workspace: Workspace, overlays: Overlays) -> impl IntoView {
    let (editing, set_editing) = signal(false);
    let (draft, set_draft) = signal(project.name.clone());

    let shown_name = StoredValue::new(project.name.clone());
    let stamp = human_time(project.updated_at);
    let row_id = StoredValue::new(project.id.clone());
    let row = StoredValue::new(project);

    let field: NodeRef<Input> = NodeRef::new();

    Effect::new(move |_| {
        if let Some(input) = field.get() {
            input.set_value(&draft.get_untracked());
            let _ = input.focus();
            input.select();
        }
    });

    let start_editing = Callback::new(move |()| set_editing.set(true));

    let commit = Callback::new(move |()| {
        let next = draft.get();
        if next.trim().is_empty() {
            return;
        }

        set_editing.set(false);
        workspace.rename(row_id.get_value(), next);
    });

    html::li().child((
        move || {
            if editing.get() {
                html::input()
                    .r#type("text")
                    .node_ref(field)
                    .prop("value", move || draft.get())
                    .on(ev::input, move |event| {
                        set_draft.set(event_target_value(&event))
                    })
                    .on(ev::keydown, move |event| {
                        if event.key() == "Enter" {
                            commit.run(());
                        }
                    })
                    .into_any()
            } else {
                html::span()
                    .class("name")
                    .child(shown_name.get_value())
                    .into_any()
            }
        },
        html::span().class("stamp").child(stamp),
        move || {
            if editing.get() {
                html::div()
                    .class("actions")
                    .child((
                        html::button()
                            .disabled(move || {
                                draft.get().trim().is_empty() || workspace.renaming().get()
                            })
                            .on(ev::click, move |_| commit.run(()))
                            .child("Save"),
                        html::button()
                            .on(ev::click, move |_| set_editing.set(false))
                            .child("Cancel"),
                    ))
                    .into_any()
            } else {
                RowMenu(RowMenuProps {
                    row,
                    overlays,
                    on_rename: start_editing,
                })
                .into_any()
            }
        },
    ))
}

#[component]
fn RowMenu(
    row: StoredValue<Project>,
    overlays: Overlays,
    on_rename: Callback<()>,
) -> impl IntoView {
    let row_id = StoredValue::new(row.get_value().id);
    let open = move || overlays.is_menu_open(&row_id.get_value());

    let kebab = view! {
        <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="8" cy="3" r="1.5" fill="currentColor" />
            <circle cx="8" cy="8" r="1.5" fill="currentColor" />
            <circle cx="8" cy="13" r="1.5" fill="currentColor" />
        </svg>
    };

    html::div()
        .class("actions")
        .on(ev::click, |event| event.stop_propagation())
        .child((
            html::button()
                .class("menu-button")
                .attr("aria-label", "More actions")
                .attr("aria-haspopup", "menu")
                .attr("aria-expanded", move || open().to_string())
                .on(ev::click, move |_| overlays.toggle_menu(row_id.get_value()))
                .child(kebab),
            move || {
                open().then(|| {
                    html::div().class("menu").role("menu").child((
                        html::button()
                            .role("menuitem")
                            .on(ev::click, move |_| {
                                overlays.close_menu();
                                on_rename.run(());
                            })
                            .child("Rename"),
                        html::button()
                            .class("danger")
                            .role("menuitem")
                            .on(ev::click, move |_| {
                                overlays.close_menu();
                                overlays.ask_to_delete(row.get_value());
                            })
                            .child("Delete"),
                    ))
                })
            },
        ))
}

fn human_time(moment: OffsetDateTime) -> String {
    let here = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let shown = format_description!("[day] [month repr:short] [year], [hour]:[minute]");

    moment
        .to_offset(here)
        .format(shown)
        .unwrap_or_else(|_| "an unknown moment".to_owned())
}
