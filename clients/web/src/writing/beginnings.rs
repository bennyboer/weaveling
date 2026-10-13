use std::collections::{HashMap, HashSet};

use leptos::prelude::*;

use crate::http::ApiError;
use crate::outline::model::{Attachment, SectionId};
use crate::outline::service;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;
use crate::writing::structure::Structure;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Scene {
        section: SectionId,
        after: Option<SceneId>,
    },
    Section {
        after: SectionId,
    },
}

#[derive(Clone, Copy)]
pub struct Beginnings {
    structure: Structure,
    problems: Problems,
    pending: RwSignal<Option<Pending>>,
    beginning_section: StoredValue<bool>,
    section_words: StoredValue<String>,
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
            pending: RwSignal::new(None),
            beginning_section: StoredValue::new(false),
            section_words: StoredValue::new(String::new()),
            focus_next: StoredValue::new(None),
            opening: StoredValue::new(HashSet::new()),
            opening_words: StoredValue::new(HashMap::new()),
            opened: StoredValue::new(HashMap::new()),
        }
    }

    pub fn start_pending_scene_after(self, scene: &SceneId) {
        if let Some(section) = self.structure.section_holding(scene) {
            self.pending.set(Some(Pending::Scene {
                section,
                after: Some(scene.clone()),
            }));
        }
    }

    pub fn start_pending_scene_first_in(self, section: &SectionId) {
        self.pending.set(Some(Pending::Scene {
            section: section.clone(),
            after: None,
        }));
    }

    pub fn start_pending_section_after(self, section: &SectionId) {
        self.pending.set(Some(Pending::Section {
            after: section.clone(),
        }));
    }

    pub fn drop_pending(self, pending: &Pending) {
        if self
            .pending
            .with_untracked(|held| held.as_ref() == Some(pending))
        {
            self.pending.set(None);
        }
    }

    pub fn pending_scene_after(self, scene: &SceneId) -> Option<Pending> {
        self.pending.with(|pending| match pending {
            Some(Pending::Scene { after, .. }) if after.as_ref() == Some(scene) => pending.clone(),
            _ => None,
        })
    }

    pub fn pending_scene_first_in(self, section: &SectionId) -> bool {
        self.pending.with(|pending| {
            matches!(
                pending,
                Some(Pending::Scene { section: held, after: None }) if held == section
            )
        })
    }

    pub fn pending_section_below(self, section: &SectionId) -> Option<Pending> {
        let after = self.pending.with(|pending| match pending {
            Some(Pending::Section { after }) => Some(after.clone()),
            _ => None,
        })?;

        (self.structure.end_of_block(&after).as_ref() == Some(section))
            .then_some(Pending::Section { after })
    }

    pub fn begin_section(self, after: Option<SectionId>) {
        let under = after
            .as_ref()
            .and_then(|after| self.structure.parent_of(after));

        self.begin_section_at(under, after);
    }

    pub fn new_section_at(self, under: Option<SectionId>, after: Option<SectionId>) {
        self.section_words.set_value(String::new());
        self.begin_section_at(under, after);
    }

    fn begin_section_at(self, under: Option<SectionId>, after: Option<SectionId>) {
        if self.beginning_section.get_value() {
            return;
        }
        let Some(outline) = self.structure.id() else {
            return;
        };
        self.beginning_section.set_value(true);

        self.structure.in_turn(async move {
            match service::add(&outline, under, after, "").await {
                Ok((section, opened)) => {
                    self.focus_next.set_value(Some(section.clone()));
                    self.pending.set(None);
                    self.structure.adopt(opened);
                    self.beginning_section.set_value(false);

                    let typed = self.section_words.get_value();
                    if !typed.trim().is_empty() {
                        self.structure.retitle(section, typed);
                    }
                }
                Err(failure) => {
                    self.beginning_section.set_value(false);
                    self.problems.report(failure);
                }
            }
        });
    }

    pub fn section_words_are(self, typed: String) {
        self.section_words.set_value(typed);
    }

    pub fn section_words(self) -> String {
        self.section_words.get_value()
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

    pub fn begin_scene(self, section: SectionId, after: Option<SceneId>) {
        let started = self
            .opening
            .try_update_value(|opening| opening.insert(section.clone()))
            .unwrap_or(false);
        let Some(outline) = self.structure.id().filter(|_| started) else {
            return;
        };
        let project = self.structure.project();

        self.structure.in_turn(async move {
            let made = match scenes::create(&project).await {
                Ok(made) => made,
                Err(failure) => return self.could_not_begin(&section, failure),
            };
            self.opened
                .update_value(|opened| _ = opened.insert(made.id.clone(), section.clone()));

            match service::attach(
                &outline,
                &Attachment::Scene(made.id),
                &section,
                after.map(Attachment::Scene),
            )
            .await
            {
                Ok(placed) => {
                    self.pending.set(None);
                    self.structure.adopt(placed);
                }
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
