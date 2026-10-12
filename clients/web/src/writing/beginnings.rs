use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::http::ApiError;
use crate::outline::model::{Attachment, SectionId};
use crate::outline::service;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;
use crate::writing::structure::Structure;

#[derive(Clone, Copy)]
pub struct Beginnings {
    structure: Structure,
    problems: Problems,
    beginning: StoredValue<bool>,
    first_words: StoredValue<String>,
    focus_next: StoredValue<Option<SectionId>>,
    opening: StoredValue<HashSet<SectionId>>,
    opening_words: StoredValue<HashMap<SectionId, String>>,
    opened: StoredValue<HashMap<SceneId, SectionId>>,
}

impl Beginnings {
    pub fn of(structure: Structure, problems: Problems) -> Self {
        Self {
            structure,
            problems,
            beginning: StoredValue::new(false),
            first_words: StoredValue::new(String::new()),
            focus_next: StoredValue::new(None),
            opening: StoredValue::new(HashSet::new()),
            opening_words: StoredValue::new(HashMap::new()),
            opened: StoredValue::new(HashMap::new()),
        }
    }

    pub fn begin(self) {
        if self.beginning.get_value() {
            return;
        }
        let Some(outline) = self.structure.id() else {
            return;
        };
        self.beginning.set_value(true);

        spawn_local(async move {
            match service::add(&outline, None, None, "").await {
                Ok((section, opened)) => {
                    self.focus_next.set_value(Some(section.clone()));
                    self.structure.adopt(opened);

                    let typed = self.first_words.get_value();
                    if !typed.trim().is_empty() {
                        self.structure.retitle(section, typed);
                    }
                }
                Err(failure) => {
                    self.beginning.set_value(false);
                    self.problems.report(failure);
                }
            }
        });
    }

    pub fn first_words_are(self, typed: String) {
        self.first_words.set_value(typed);
    }

    pub fn first_words(self) -> String {
        self.first_words.get_value()
    }

    pub fn takes_focus(self, section: &SectionId) -> bool {
        let wanted = self
            .focus_next
            .with_value(|next| next.as_ref() == Some(section));

        if wanted {
            self.focus_next.set_value(None);
        }

        wanted
    }

    pub fn begin_scene(self, section: SectionId) {
        let started = self
            .opening
            .try_update_value(|opening| opening.insert(section.clone()))
            .unwrap_or(false);
        let Some(outline) = self.structure.id().filter(|_| started) else {
            return;
        };
        let project = self.structure.project();

        spawn_local(async move {
            let made = match scenes::create(&project).await {
                Ok(made) => made,
                Err(failure) => return self.could_not_begin(&section, failure),
            };
            self.opened
                .update_value(|opened| _ = opened.insert(made.id.clone(), section.clone()));

            match service::attach(&outline, &Attachment::Scene(made.id), &section, None).await {
                Ok(placed) => self.structure.adopt(placed),
                Err(failure) => self.could_not_begin(&section, failure),
            }
        });
    }

    pub fn opening_words_are(self, section: SectionId, typed: String) {
        self.opening_words
            .update_value(|words| _ = words.insert(section, typed));
    }

    pub fn words_to_carry(self, scene: &SceneId) -> Option<String> {
        let section = self
            .opened
            .try_update_value(|opened| opened.remove(scene))
            .flatten()?;
        self.opening
            .update_value(|opening| _ = opening.remove(&section));

        self.opening_words
            .try_update_value(|words| words.remove(&section))
            .flatten()
    }

    fn could_not_begin(self, section: &SectionId, failure: ApiError) {
        self.opening
            .update_value(|opening| _ = opening.remove(section));
        self.problems.report(failure);
    }
}
