use leptos::prelude::*;
use web_sys::Storage;

use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::writing::loose_scenes::LooseScenes;
use crate::writing::side::Side;
use crate::writing::structure::Structure;
use crate::writing::viewport::Viewport;
use crate::writing::walk::{Stop, Towards, Walk, stops_in_reading_order};

#[derive(Clone, Copy)]
pub struct LeftOff {
    project: StoredValue<ProjectId>,
    structure: Structure,
    walk: Walk,
    viewport: Viewport,
    side: Side,
    loose: LooseScenes,
}

impl LeftOff {
    pub fn in_project(
        project: &ProjectId,
        side: Side,
        structure: Structure,
        loose: LooseScenes,
        walk: Walk,
        viewport: Viewport,
    ) -> Self {
        Self {
            project: StoredValue::new(project.clone()),
            structure,
            walk,
            viewport,
            side,
            loose,
        }
    }

    fn land_in(self, scene: SceneId, at: u32) {
        self.viewport.hold_for_landing(scene.clone());
        self.walk.land_in(scene, at);
    }

    pub fn remember(self, scene: &SceneId, at: u32) {
        if let Some(shelf) = shelf() {
            let _ = shelf.set_item(&self.key(), &format!("{scene} {at}"));
        }
    }

    pub fn land(self, following: Option<SceneId>) {
        let recalled = self.recalled();

        if let Some(scene) = following {
            let at = recalled
                .filter(|(left_in, _)| left_in == &scene)
                .map_or(0, |(_, at)| at);
            self.land_in(scene, at);
            return;
        }

        let recalled = recalled.filter(|(scene, _)| match self.side {
            Side::Book => self.structure.holds(scene),
            Side::Loose => self.loose.in_order_untracked().contains(scene),
        });

        if let Some((scene, at)) = recalled {
            self.land_in(scene, at);
            return;
        }

        if self.side == Side::Loose {
            if let Some(first) = self.loose.in_order_untracked().into_iter().next() {
                self.land_in(first, 0);
            }
            return;
        }

        let read = self
            .structure
            .read_untracked(stops_in_reading_order)
            .unwrap_or_default();
        let first_scene = read.iter().find_map(|stop| match stop {
            Stop::Scene(scene) => Some(scene.clone()),
            _ => None,
        });

        match first_scene {
            Some(scene) => self.land_in(scene, 0),
            None => {
                if let Some(opening) = read
                    .into_iter()
                    .find(|stop| matches!(stop, Stop::Opening(_)))
                    && !self.walk.arrive(&opening, Towards::After)
                {
                    self.walk.arrive_when_drawn(opening);
                }
            }
        }
    }

    fn recalled(self) -> Option<(SceneId, u32)> {
        let kept = shelf()?.get_item(&self.key()).ok()??;
        let (scene, at) = kept.split_once(' ')?;

        Some((SceneId::from(scene.to_owned()), at.parse().ok()?))
    }

    fn key(self) -> String {
        match self.side {
            Side::Book => format!("weaveling.left-off.{}", self.project.get_value()),
            Side::Loose => format!("weaveling.left-off.{}.loose", self.project.get_value()),
        }
    }
}

fn shelf() -> Option<Storage> {
    window().local_storage().ok().flatten()
}
