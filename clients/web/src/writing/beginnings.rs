use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task;

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;
use crate::writing::scene_copies::SceneCopies;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unplaced {
    Scene { scene: SceneId, begins: String },
    Section { titled: String },
}

#[derive(Clone, Copy)]
pub struct Beginnings {
    structure: Structure,
    scenes: SceneCopies,
    problems: Problems,
    pending: RwSignal<Option<Pending>>,
    beginning_section: StoredValue<bool>,
    section_words: StoredValue<String>,
    focus_next: StoredValue<Option<SectionId>>,
    opening: RwSignal<HashSet<SectionId>>,
    opening_words: StoredValue<HashMap<SectionId, String>>,
    opened: StoredValue<HashMap<SceneId, SectionId>>,
    unplaced: RwSignal<Option<Unplaced>>,
}

impl Beginnings {
    pub fn of(structure: Structure, scenes: SceneCopies, problems: Problems) -> Self {
        Self {
            structure,
            scenes,
            problems,
            pending: RwSignal::new(None),
            beginning_section: StoredValue::new(false),
            section_words: StoredValue::new(String::new()),
            focus_next: StoredValue::new(None),
            opening: RwSignal::new(HashSet::new()),
            opening_words: StoredValue::new(HashMap::new()),
            opened: StoredValue::new(HashMap::new()),
            unplaced: RwSignal::new(None),
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
        if self.beginning_section.get_value() || self.structure.id().is_none() {
            return;
        }
        self.beginning_section.set_value(true);
        let adding = self.structure.added(under, after.clone(), move |section| {
            _ = self.focus_next.try_set_value(Some(section));
            _ = self.pending.try_set(None);
        });

        task::spawn_local(async move {
            let added = adding.await;
            _ = self.beginning_section.try_set_value(false);
            match added {
                Ok(section) => {
                    let typed = self.section_words.get_value();
                    if !typed.trim().is_empty() {
                        self.structure.retitle(section, typed);
                    }
                }
                Err(failure) => {
                    let gone = after
                        .as_ref()
                        .is_some_and(|after| self.structure.title_of(after).is_none());
                    if !gone {
                        return self.structure.refused(failure);
                    }
                    self.pending.set(None);
                    self.unplaced.set(Some(Unplaced::Section {
                        titled: self.section_words.get_value(),
                    }));
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
            .try_update(|opening| opening.insert(section.clone()))
            .unwrap_or(false);
        if !started || self.structure.id().is_none() {
            return;
        }
        let project = self.structure.project();

        task::spawn_local(async move {
            let made = match scenes::create(&project).await {
                Ok(made) => made,
                Err(failure) => {
                    self.opening.update(|opening| _ = opening.remove(&section));
                    return self.problems.report(failure);
                }
            };
            self.opened
                .update_value(|opened| _ = opened.insert(made.id.clone(), section.clone()));
            let scene = made.id.clone();
            self.scenes.hold(made);

            let Err(failure) = self
                .structure
                .brought_back(scene.clone(), section.clone(), after)
                .await
            else {
                return;
            };
            self.opening.update(|opening| _ = opening.remove(&section));
            if self.structure.title_of(&section).is_some() {
                return self.structure.refused(failure);
            }
            let begins = self
                .opening_words
                .with_value(|words| words.get(&section).cloned())
                .unwrap_or_default();
            self.unplaced.set(Some(Unplaced::Scene { scene, begins }));
        });
    }

    pub fn unplaced(self) -> Option<Unplaced> {
        self.unplaced.get()
    }

    pub fn let_unplaced_be_asked(self) {
        self.unplaced.set(None);
    }

    pub fn put_at_the_end(self, unplaced: Unplaced) {
        match unplaced {
            Unplaced::Scene { scene, .. } => {
                if let Some(last) = self.structure.last_section() {
                    let after = self.structure.scenes_of(&last).last().cloned();
                    self.structure.bring_back(scene, last, after);
                }
            }
            Unplaced::Section { .. } => {
                let after = self.structure.top_level().last().cloned();
                self.begin_section_at(None, after);
            }
        }
    }

    pub fn discard(self, unplaced: Unplaced) {
        match unplaced {
            Unplaced::Scene { scene, .. } => {
                if let Some(section) = self
                    .opened
                    .try_update_value(|opened| opened.remove(&scene))
                    .flatten()
                {
                    self.opening_words
                        .update_value(|words| _ = words.remove(&section));
                }
                let deleting = self.scenes.deleted(scene);
                task::spawn_local(async move {
                    deleting.await;
                });
            }
            Unplaced::Section { .. } => {}
        }
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
        self.opening.update(|opening| _ = opening.remove(&section));
        if self.pending.with_untracked(Option::is_some) {
            self.pending.set(None);
        }

        self.opening_words
            .try_update_value(|words| words.remove(&section))
            .flatten()
    }

    pub fn hands_over(self, section: &SectionId) -> bool {
        self.opening.with(|opening| opening.contains(section))
    }
}
