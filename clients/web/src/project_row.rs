use leptos::html::Input;
use leptos::prelude::*;
use projects_contract::ProjectDTO;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

use crate::overlays::Overlays;
use crate::workspace::Workspace;

#[component]
pub fn ProjectRow(project: ProjectDTO, workspace: Workspace, overlays: Overlays) -> impl IntoView {
    let (editing, set_editing) = signal(false);
    let (draft, set_draft) = signal(project.name.clone());

    let shown_name = project.name.clone();
    let stamp = human_time(&project.updated_at);
    let row_id = StoredValue::new(project.id.clone());
    let row = StoredValue::new(project);

    let editor: NodeRef<Input> = NodeRef::new();

    Effect::new(move |_| {
        if let Some(field) = editor.get() {
            field.set_value(&draft.get_untracked());
            let _ = field.focus();
            field.select();
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

    view! {
        <li>
            <Show
                when=move || editing.get()
                fallback=move || {
                    let name = shown_name.clone();
                    view! { <span class="name">{name}</span> }
                }
            >
                <input
                    type="text"
                    node_ref=editor
                    prop:value=move || draft.get()
                    on:input=move |event| set_draft.set(event_target_value(&event))
                    on:keydown=move |event| {
                        if event.key() == "Enter" {
                            commit.run(());
                        }
                    }
                />
            </Show>

            <span class="stamp">{stamp}</span>

            <Show
                when=move || editing.get()
                fallback=move || {
                    view! { <RowMenu row=row overlays=overlays on_rename=start_editing /> }
                }
            >
                <div class="actions">
                    <button
                        disabled=move || draft.get().trim().is_empty()
                        on:click=move |_| commit.run(())
                    >
                        "Save"
                    </button>
                    <button on:click=move |_| set_editing.set(false)>"Cancel"</button>
                </div>
            </Show>
        </li>
    }
}

#[component]
fn RowMenu(
    row: StoredValue<ProjectDTO>,
    overlays: Overlays,
    on_rename: Callback<()>,
) -> impl IntoView {
    let row_id = StoredValue::new(row.get_value().id);
    let open = move || overlays.is_menu_open(&row_id.get_value());

    view! {
        <div class="actions" on:click=move |event| event.stop_propagation()>
            <button
                class="menu-button"
                aria-label="More actions"
                aria-haspopup="menu"
                aria-expanded=move || open().to_string()
                on:click=move |_| overlays.toggle_menu(row_id.get_value())
            >
                <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
                    <circle cx="8" cy="3" r="1.5" fill="currentColor" />
                    <circle cx="8" cy="8" r="1.5" fill="currentColor" />
                    <circle cx="8" cy="13" r="1.5" fill="currentColor" />
                </svg>
            </button>

            <Show when=open fallback=|| ()>
                <div class="menu" role="menu">
                    <button
                        role="menuitem"
                        on:click=move |_| {
                            overlays.close_menu();
                            on_rename.run(());
                        }
                    >
                        "Rename"
                    </button>
                    <button
                        class="danger"
                        role="menuitem"
                        on:click=move |_| {
                            overlays.close_menu();
                            overlays.ask_to_delete(row.get_value());
                        }
                    >
                        "Delete"
                    </button>
                </div>
            </Show>
        </div>
    }
}

fn human_time(raw: &str) -> String {
    let Ok(moment) = OffsetDateTime::parse(raw, &Rfc3339) else {
        return raw.to_owned();
    };
    let here = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let shown = format_description!("[day] [month repr:short] [year], [hour]:[minute]");

    moment
        .to_offset(here)
        .format(shown)
        .unwrap_or_else(|_| raw.to_owned())
}
