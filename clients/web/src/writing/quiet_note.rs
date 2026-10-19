use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons::{Icon, mark};
use crate::writing::write_view_state::WriteViewState;

#[derive(Clone, Copy)]
pub struct QuietNote {
    told: RwSignal<Option<String>>,
}

impl QuietNote {
    pub fn none() -> Self {
        Self {
            told: RwSignal::new(None),
        }
    }

    pub fn tell(self, saying: String) {
        self.told.set(Some(saying));
    }

    pub fn dismiss(self) {
        self.told.set(None);
    }

    pub fn told(self) -> Option<String> {
        self.told.get()
    }
}

pub fn quiet_note(state: WriteViewState) -> impl IntoView {
    Effect::new(move || {
        let rearranged = state.structure.rearranged_by_others();
        if rearranged.is_empty() {
            return;
        }
        let mut around = untrack(|| state.writing_in.last_written_in());
        while let Some(section) = around {
            if rearranged.contains(&section) {
                let title = untrack(|| state.structure.title_of(&section)).unwrap_or_default();
                state.quiet_note.tell(format!(
                    "Someone else rearranged the book around “{}”.",
                    title.trim()
                ));
                return;
            }
            around = state.structure.parent_of(&section);
        }
    });

    move || {
        state.quiet_note.told().map(|saying| {
            html::div().class("quiet-note").role("status").child((
                html::span().child(saying),
                html::button()
                    .r#type("button")
                    .attr("aria-label", "Dismiss")
                    .attr("title", "Dismiss")
                    .on(ev::click, move |_| state.quiet_note.dismiss())
                    .child(mark(Icon::Remove)),
            ))
        })
    }
}
