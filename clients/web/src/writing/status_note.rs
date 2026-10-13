use std::time::Duration;

use leptos::prelude::*;
use leptos::{IntoView, html};

use crate::icons::{Icon, mark};
use crate::writing::write_view_state::WriteViewState;

const SETTLE: Duration = Duration::from_millis(500);

pub fn status_note(state: WriteViewState) -> impl IntoView {
    let shown_saving = RwSignal::new(false);
    let settling = StoredValue::new(None::<TimeoutHandle>);

    Effect::new(move || {
        let saving = state.saving.is_saving();

        if let Some(handle) = settling.get_value() {
            handle.clear();
        }
        settling.set_value(None);

        if saving {
            shown_saving.set(true);
        } else {
            settling.set_value(set_timeout(move || _ = shown_saving.try_set(false), SETTLE).ok());
        }
    });
    on_cleanup(move || {
        if let Some(handle) = settling.get_value() {
            handle.clear();
        }
    });

    html::p().class("status-note").child(move || {
        if shown_saving.get() {
            (mark(Icon::Saving), html::span().child("Saving…"))
        } else {
            (mark(Icon::Saved), html::span().child("All saved"))
        }
    })
}
