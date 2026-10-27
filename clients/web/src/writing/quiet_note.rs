use leptos::prelude::*;

use crate::speech;
use crate::writing::asking::Asking;
use crate::writing::write_view_state::WriteViewState;

pub fn told_quietly(state: WriteViewState) {
    Effect::new(move || {
        let rearranged = state.structure.rearranged_by_others();
        if rearranged.is_empty() {
            return;
        }
        let mut around = untrack(|| state.writing_in.last_written_in());
        while let Some(section) = around {
            if rearranged.contains(&section) {
                let title = untrack(|| state.structure.title_of(&section)).unwrap_or_default();
                state.quiet_note.tell(speech::say_about(
                    "quiet-rearranged",
                    "section",
                    title.trim(),
                ));
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
            None => state.quiet_note.tell(speech::say_about(
                "quiet-removed",
                "section",
                mine.title.trim(),
            )),
        }
    });

    Effect::new(move |told_before: Option<u64>| {
        let times = state.structure.overtaken();
        if told_before.is_some_and(|before| before < times) {
            state.quiet_note.tell(speech::say("quiet-overtaken"));
        }
        times
    });
}
