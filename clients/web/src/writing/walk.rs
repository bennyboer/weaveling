use std::collections::HashMap;

use leptos::prelude::*;
use web_sys::HtmlElement;

use crate::outline::model::{Outline, SectionId};
use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;
use crate::writing::loose_scenes::LooseScenes;
use crate::writing::side::Side;
use crate::writing::structure::Structure;
use crate::writing::viewport::Viewport;

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
    awaited: StoredValue<Option<Stop>>,
    landing: StoredValue<Option<(SceneId, u32)>>,
    arriving: StoredValue<Option<(SceneId, Towards)>>,
    viewport: Viewport,
    side: Side,
    loose: LooseScenes,
}

impl Walk {
    pub fn through(
        structure: Structure,
        viewport: Viewport,
        side: Side,
        loose: LooseScenes,
    ) -> Self {
        Self {
            structure,
            side,
            loose,
            editors: StoredValue::new_local(HashMap::new()),
            lines: StoredValue::new_local(HashMap::new()),
            awaited: StoredValue::new(None),
            landing: StoredValue::new(None),
            arriving: StoredValue::new(None),
            viewport,
        }
    }

    pub fn writing_in(self, scene: SceneId, editor: ProseEditor) {
        let landing = self
            .landing
            .try_update_value(|landing| {
                landing
                    .take_if(|(awaited, _)| awaited == &scene)
                    .map(|(_, at)| at)
            })
            .flatten();

        if let Some(at) = landing {
            editor.land_at(at);
        }
        let arriving = self
            .arriving
            .try_update_value(|arriving| {
                arriving
                    .take_if(|(awaited, _)| awaited == &scene)
                    .map(|(_, towards)| towards)
            })
            .flatten();
        if let Some(towards) = arriving {
            editor.arrive_when_synced(match towards {
                Towards::Before => "before",
                Towards::After => "after",
            });
        }
        self.editors
            .update_value(|editors| _ = editors.insert(scene, editor));
    }

    pub fn land_in(self, scene: SceneId, at: u32) {
        let landed = self.editors.with_value(|editors| {
            editors
                .get(&scene)
                .map(|editor| editor.land_at(at))
                .is_some()
        });

        if !landed {
            self.landing.set_value(Some((scene, at)));
        }
    }

    pub fn told(self, scene: &SceneId, told: impl FnOnce(&ProseEditor)) {
        _ = self
            .editors
            .try_with_value(|editors| editors.get(scene).map(told));
    }

    pub fn text_in(self, scene: &SceneId) -> Option<String> {
        self.editors
            .try_with_value(|editors| editors.get(scene).map(ProseEditor::plain_text))
            .flatten()
    }

    pub fn back_into(self, scene: &SceneId) -> bool {
        self.editors
            .with_value(|editors| editors.get(scene).map(|editor| editor.focus()).is_some())
    }

    pub fn done_with(self, scene: &SceneId) -> Option<ProseEditor> {
        self.editors
            .try_update_value(|editors| editors.remove(scene))
            .flatten()
    }

    pub fn line_at(self, stop: Stop, shown: HtmlElement) {
        let awaited = self
            .awaited
            .with_value(|awaited| awaited.as_ref() == Some(&stop));

        if awaited {
            self.awaited.set_value(None);
            caret_in(&shown, Towards::Before);
        }
        self.lines
            .update_value(|lines| _ = lines.insert(stop, shown));
    }

    pub fn arrive_when_drawn(self, stop: Stop) {
        self.awaited.set_value(Some(stop));
    }

    pub fn line_gone(self, stop: &Stop) {
        self.lines.update_value(|lines| _ = lines.remove(stop));
    }

    fn reading_order(self) -> Vec<Stop> {
        match self.side {
            Side::Book => self
                .structure
                .read_untracked(stops_in_reading_order)
                .unwrap_or_default(),
            Side::Loose => self
                .loose
                .in_order_untracked()
                .into_iter()
                .map(Stop::Scene)
                .collect(),
        }
    }

    pub fn leave(self, from: &Stop, towards: Towards) -> bool {
        let read = self.reading_order();

        neighbour(&read, from, towards).is_some_and(|next| self.arrive(&next, towards))
    }

    pub fn arrive(self, at: &Stop, towards: Towards) -> bool {
        match at {
            Stop::Scene(scene) => {
                let mounted = self.editors.with_value(|editors| match editors.get(scene) {
                    Some(editor) => {
                        match towards {
                            Towards::Before => editor.focus_end(),
                            Towards::After => editor.focus_start(),
                        }

                        true
                    }
                    None => false,
                });

                if !mounted {
                    self.arriving.set_value(Some((scene.clone(), towards)));
                    self.viewport.hold_for_landing(scene.clone());
                }

                true
            }
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
