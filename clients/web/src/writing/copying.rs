use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use web_sys::ClipboardEvent;

use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::writing::choosing::Choosing;
use crate::writing::problems::Problems;
use crate::writing::structure::Structure;
use crate::writing::walk::{Stop, Walk};

const BETWEEN: &str = "\n\n";

#[derive(Clone, Copy)]
pub struct Copying {
    choosing: Choosing,
    structure: Structure,
    walk: Walk,
    problems: Problems,
    fetched: StoredValue<HashMap<SceneId, String>>,
}

impl Copying {
    pub fn of(choosing: Choosing, structure: Structure, walk: Walk, problems: Problems) -> Self {
        Self {
            choosing,
            structure,
            walk,
            problems,
            fetched: StoredValue::new(HashMap::new()),
        }
    }

    pub fn copied(self, event: &ClipboardEvent) {
        if !self.choosing.is_active() {
            return;
        }
        event.prevent_default();

        match (self.chosen_text(), event.clipboard_data()) {
            (Some(text), Some(clipboard)) => _ = clipboard.set_data("text/plain", &text),
            _ => self.copy_once_fetched(),
        }
    }

    fn copy_once_fetched(self) {
        spawn_local(async move {
            let missing = self.not_at_hand();
            self.fetch(missing).await;

            if let Some(text) = self.chosen_text() {
                _ = window().navigator().clipboard().write_text(&text);
            }
        });
    }

    fn not_at_hand(self) -> Vec<SceneId> {
        self.choosing
            .in_reading_order()
            .into_iter()
            .filter_map(|stop| match stop {
                Stop::Scene(scene) if self.text_of(&scene).is_none() => Some(scene),
                _ => None,
            })
            .collect()
    }

    async fn fetch(self, scenes_wanted: Vec<SceneId>) {
        for scene in scenes_wanted {
            match scenes::open(&scene).await {
                Ok(found) => {
                    _ = self
                        .fetched
                        .try_update_value(|fetched| fetched.insert(scene, found.text));
                }
                Err(failure) => self.problems.report(failure),
            }
        }
    }

    fn text_of(self, scene: &SceneId) -> Option<String> {
        self.walk.text_in(scene).or_else(|| {
            self.fetched
                .try_with_value(|fetched| fetched.get(scene).cloned())
                .flatten()
        })
    }

    fn chosen_text(self) -> Option<String> {
        self.choosing
            .in_reading_order()
            .into_iter()
            .map(|stop| match stop {
                Stop::Scene(scene) => self.text_of(&scene),
                Stop::Heading(section) | Stop::Opening(section) => {
                    Some(untrack(|| self.structure.title_of(&section)).unwrap_or_default())
                }
            })
            .collect::<Option<Vec<_>>>()
            .map(|parts| parts.join(BETWEEN))
    }
}
