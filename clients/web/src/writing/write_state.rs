use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use web_sys::HtmlElement;

use crate::http::ApiError;
use crate::outline::model::{Outline, OutlineId, Section, SectionId};
use crate::outline::service;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Towards {
    Before,
    After,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    Heading(SectionId),
    Scene(SceneId),
}

#[derive(Clone, Copy)]
pub struct WriteState {
    outline: RwSignal<Option<Outline>>,
    problem: RwSignal<Option<ApiError>>,
    editors: StoredValue<HashMap<SceneId, ProseEditor>, LocalStorage>,
    headings: StoredValue<HashMap<SectionId, HtmlElement>, LocalStorage>,
    beginning: StoredValue<bool>,
    first_words: StoredValue<String>,
    focus_next: StoredValue<Option<SectionId>>,
}

impl WriteState {
    pub fn open(project: &ProjectId) -> Self {
        let state = Self {
            outline: RwSignal::new(None),
            problem: RwSignal::new(None),
            editors: StoredValue::new_local(HashMap::new()),
            headings: StoredValue::new_local(HashMap::new()),
            beginning: StoredValue::new(false),
            first_words: StoredValue::new(String::new()),
            focus_next: StoredValue::new(None),
        };
        let project = project.clone();

        spawn_local(async move {
            match service::open(&project).await {
                Ok(opened) => state.outline.set(Some(opened)),
                Err(failure) => state.problem.set(Some(failure)),
            }
        });

        state
    }

    pub fn is_open(self) -> bool {
        self.outline.with(Option::is_some)
    }

    pub fn is_unshaped(self) -> bool {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .is_some_and(|book| book.sections.is_empty())
        })
    }

    pub fn sections(self) -> Vec<(usize, Section)> {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .map(|book| {
                    book.in_reading_order()
                        .into_iter()
                        .map(|(depth, section)| (depth, section.clone()))
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    pub fn scenes_of(self, section: &SectionId) -> Vec<SceneId> {
        self.outline.with(|outline| {
            outline
                .as_ref()
                .and_then(|book| book.sections.iter().find(|held| &held.id == section))
                .map(Section::scenes)
                .unwrap_or_default()
        })
    }

    pub fn problem(self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn begin(self) {
        if self.beginning.get_value() {
            return;
        }
        let Some(outline) = self.outline_id() else {
            return;
        };
        self.beginning.set_value(true);

        spawn_local(async move {
            match service::add(&outline, None, None, "").await {
                Ok((section, opened)) => {
                    self.focus_next.set_value(Some(section.clone()));
                    self.adopt(opened);

                    let typed = self.first_words.get_value();
                    if !typed.trim().is_empty() {
                        self.retitle(section, typed);
                    }
                }
                Err(failure) => {
                    self.beginning.set_value(false);
                    self.problem.set(Some(failure));
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

    pub fn retitle(self, section: SectionId, title: String) {
        let Some(outline) = self.outline_id() else {
            return;
        };

        spawn_local(async move {
            match service::retitle(&outline, &section, &title).await {
                Ok(renamed) => self.adopt(renamed),
                Err(failure) => self.problem.set(Some(failure)),
            }
        });
    }

    fn outline_id(self) -> Option<OutlineId> {
        self.outline
            .with_untracked(|outline| outline.as_ref().map(|book| book.id.clone()))
    }

    fn adopt(self, arrived: Outline) {
        let newer = self.outline.with_untracked(|held| {
            held.as_ref()
                .is_none_or(|held| arrived.version >= held.version)
        });

        if newer {
            self.outline.set(Some(arrived));
        }
    }

    pub fn writing_in(self, scene: SceneId, editor: ProseEditor) {
        self.editors
            .update_value(|editors| _ = editors.insert(scene, editor));
    }

    pub fn done_with(self, scene: &SceneId) -> Option<ProseEditor> {
        self.editors
            .try_update_value(|editors| editors.remove(scene))
            .flatten()
    }

    pub fn heading_at(self, section: SectionId, shown: HtmlElement) {
        self.headings
            .update_value(|headings| _ = headings.insert(section, shown));
    }

    pub fn heading_gone(self, section: &SectionId) {
        self.headings
            .update_value(|headings| _ = headings.remove(section));
    }

    pub fn leave(self, from: &Stop, towards: Towards) -> bool {
        let read = self.outline.with_untracked(|outline| {
            outline
                .as_ref()
                .map(stops_in_reading_order)
                .unwrap_or_default()
        });
        let Some(next) = neighbour(&read, from, towards) else {
            return false;
        };

        match next {
            Stop::Scene(scene) => self
                .editors
                .with_value(|editors| match editors.get(&scene) {
                    Some(editor) => {
                        match towards {
                            Towards::Before => editor.focus_end(),
                            Towards::After => editor.focus_start(),
                        }

                        true
                    }
                    None => false,
                }),
            Stop::Heading(section) => {
                self.headings
                    .with_value(|headings| match headings.get(&section) {
                        Some(shown) => {
                            caret_in(shown, towards);

                            true
                        }
                        None => false,
                    })
            }
        }
    }
}

fn caret_in(shown: &HtmlElement, towards: Towards) {
    let _ = shown.focus();

    if let Some(chosen) = window().get_selection().ok().flatten() {
        let _ = chosen.select_all_children(shown);
        let _ = match towards {
            Towards::Before => chosen.collapse_to_end(),
            Towards::After => chosen.collapse_to_start(),
        };
    }
}

pub fn stops_in_reading_order(outline: &Outline) -> Vec<Stop> {
    outline
        .in_reading_order()
        .into_iter()
        .flat_map(|(_, section)| {
            std::iter::once(Stop::Heading(section.id.clone()))
                .chain(section.scenes().into_iter().map(Stop::Scene))
        })
        .collect()
}

fn neighbour(read: &[Stop], from: &Stop, towards: Towards) -> Option<Stop> {
    let here = read.iter().position(|held| held == from)?;

    match towards {
        Towards::Before => here.checked_sub(1).and_then(|back| read.get(back)),
        Towards::After => read.get(here + 1),
    }
    .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::model::{Attachment, OutlineId};

    fn a_section(id: &str, parent: Option<&str>, scenes: &[&str]) -> Section {
        Section {
            id: SectionId::from(id.to_owned()),
            parent: parent.map(|parent| SectionId::from(parent.to_owned())),
            title: id.to_owned(),
            attachments: scenes
                .iter()
                .map(|scene| Attachment::Scene(SceneId::from((*scene).to_owned())))
                .collect(),
        }
    }

    fn a_book() -> Outline {
        Outline {
            id: OutlineId::from("outline_1".to_owned()),
            version: 1,
            sections: vec![
                a_section("part_1", None, &["scene_1"]),
                a_section("part_2", None, &[]),
                a_section("chapter_1", Some("part_1"), &["scene_2", "scene_3"]),
            ],
        }
    }

    fn heading(id: &str) -> Stop {
        Stop::Heading(SectionId::from(id.to_owned()))
    }

    fn scene(id: &str) -> Stop {
        Stop::Scene(SceneId::from(id.to_owned()))
    }

    #[test]
    fn the_cursor_walks_each_heading_before_the_scenes_under_it() {
        assert_eq!(
            stops_in_reading_order(&a_book()),
            vec![
                heading("part_1"),
                scene("scene_1"),
                heading("chapter_1"),
                scene("scene_2"),
                scene("scene_3"),
                heading("part_2"),
            ]
        );
    }

    #[test]
    fn a_section_with_no_scenes_leads_straight_on_to_the_next_heading() {
        let read = stops_in_reading_order(&a_book());

        assert_eq!(
            neighbour(&read, &scene("scene_3"), Towards::After),
            Some(heading("part_2"))
        );
        assert_eq!(
            neighbour(&read, &heading("chapter_1"), Towards::Before),
            Some(scene("scene_1"))
        );
        assert_eq!(neighbour(&read, &heading("part_1"), Towards::Before), None);
        assert_eq!(neighbour(&read, &heading("part_2"), Towards::After), None);
    }
}
