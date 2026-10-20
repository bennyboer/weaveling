use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons::{Icon, mark};
use crate::speech::{said, say, say_about};
use crate::writing::asking::Asking;
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
                state
                    .quiet_note
                    .tell(say_about("quiet-rearranged", "section", title.trim()));
                return;
            }
            around = state.structure.parent_of(&section);
        }
    });

    Effect::new(move || {
        let removed = state.structure.removed_by_others();
        let Some((section, scene)) = untrack(|| state.writing_in.was_writing_in()) else {
            return;
        };
        let Some(mine) = removed.into_iter().find(|gone| gone.section == section) else {
            return;
        };
        match scene {
            Some(scene) => state.margin.ask(Asking::RemovedMeanwhile {
                title: mine.title,
                scene,
            }),
            None => state
                .quiet_note
                .tell(say_about("quiet-removed", "section", mine.title.trim())),
        }
    });

    Effect::new(move |told_before: Option<u64>| {
        let times = state.structure.overtaken();
        if told_before.is_some_and(|before| before < times) {
            state.quiet_note.tell(say("quiet-overtaken"));
        }
        times
    });

    move || {
        state.quiet_note.told().map(|saying| {
            html::div().class("quiet-note").role("status").child((
                html::span().child(saying),
                html::button()
                    .r#type("button")
                    .attr("aria-label", said("problem-dismiss"))
                    .attr("title", said("problem-dismiss"))
                    .on(ev::click, move |_| state.quiet_note.dismiss())
                    .child(mark(Icon::Remove)),
            ))
        })
    }
}
