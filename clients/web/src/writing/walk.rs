use std::collections::HashMap;

use leptos::prelude::*;
use web_sys::HtmlElement;

use crate::outline::model::{Outline, SectionId};
use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;
use crate::writing::structure::Structure;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Towards {
    Before,
    After,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Stop {
    Heading(SectionId),
    Scene(SceneId),
    Opening(SectionId),
}

#[derive(Clone, Copy)]
pub struct Walk {
    structure: Structure,
    editors: StoredValue<HashMap<SceneId, ProseEditor>, LocalStorage>,
    lines: StoredValue<HashMap<Stop, HtmlElement>, LocalStorage>,
}

impl Walk {
    pub fn through(structure: Structure) -> Self {
        Self {
            structure,
            editors: StoredValue::new_local(HashMap::new()),
            lines: StoredValue::new_local(HashMap::new()),
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

    pub fn line_at(self, stop: Stop, shown: HtmlElement) {
        self.lines
            .update_value(|lines| _ = lines.insert(stop, shown));
    }

    pub fn line_gone(self, stop: &Stop) {
        self.lines.update_value(|lines| _ = lines.remove(stop));
    }

    pub fn leave(self, from: &Stop, towards: Towards) -> bool {
        let read = self
            .structure
            .read_untracked(stops_in_reading_order)
            .unwrap_or_default();

        neighbour(&read, from, towards).is_some_and(|next| self.arrive(&next, towards))
    }

    pub fn arrive(self, at: &Stop, towards: Towards) -> bool {
        match at {
            Stop::Scene(scene) => self.editors.with_value(|editors| match editors.get(scene) {
                Some(editor) => {
                    match towards {
                        Towards::Before => editor.focus_end(),
                        Towards::After => editor.focus_start(),
                    }

                    true
                }
                None => false,
            }),
            line => self.lines.with_value(|lines| match lines.get(line) {
                Some(shown) => {
                    caret_in(shown, towards);

                    true
                }
                None => false,
            }),
        }
    }

    pub fn last_stop_in(self, section: &SectionId) -> Option<Stop> {
        let read = self
            .structure
            .read_untracked(stops_in_reading_order)
            .unwrap_or_default();
        let start = read
            .iter()
            .position(|stop| stop == &Stop::Heading(section.clone()))?;

        read[start..]
            .iter()
            .enumerate()
            .take_while(|(nth, stop)| *nth == 0 || !matches!(stop, Stop::Heading(_)))
            .last()
            .map(|(_, stop)| stop.clone())
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
            let scenes = section.scenes();
            let opening = scenes.is_empty().then(|| Stop::Opening(section.id.clone()));

            std::iter::once(Stop::Heading(section.id.clone()))
                .chain(scenes.into_iter().map(Stop::Scene))
                .chain(opening)
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
    use crate::outline::model::{Attachment, OutlineId, Section};

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

    fn opening(id: &str) -> Stop {
        Stop::Opening(SectionId::from(id.to_owned()))
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
                opening("part_2"),
            ]
        );
    }

    #[test]
    fn a_section_with_no_scenes_offers_a_line_to_begin_one() {
        let read = stops_in_reading_order(&a_book());

        assert_eq!(
            neighbour(&read, &scene("scene_3"), Towards::After),
            Some(heading("part_2"))
        );
        assert_eq!(
            neighbour(&read, &heading("part_2"), Towards::After),
            Some(opening("part_2")),
            "an empty section is where a scene is begun, so the cursor has to reach it"
        );
        assert_eq!(
            neighbour(&read, &heading("chapter_1"), Towards::Before),
            Some(scene("scene_1"))
        );
        assert_eq!(neighbour(&read, &heading("part_1"), Towards::Before), None);
        assert_eq!(neighbour(&read, &opening("part_2"), Towards::After), None);
    }
}
