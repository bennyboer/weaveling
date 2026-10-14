use leptos::prelude::*;
use web_sys::Storage;

use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::writing::structure::Structure;
use crate::writing::walk::{Stop, Towards, Walk, stops_in_reading_order};

#[derive(Clone, Copy)]
pub struct LeftOff {
    project: StoredValue<ProjectId>,
    structure: Structure,
    walk: Walk,
}

impl LeftOff {
    pub fn in_project(project: &ProjectId, structure: Structure, walk: Walk) -> Self {
        Self {
            project: StoredValue::new(project.clone()),
            structure,
            walk,
        }
    }

    pub fn remember(self, scene: &SceneId, at: u32) {
        if let Some(shelf) = shelf() {
            let _ = shelf.set_item(&self.key(), &format!("{scene} {at}"));
        }
    }

    pub fn land(self) {
        let recalled = self
            .recalled()
            .filter(|(scene, _)| self.structure.holds(scene));

        if let Some((scene, at)) = recalled {
            self.walk.land_in(scene, at);
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
            Some(scene) => self.walk.land_in(scene, 0),
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
        format!("weaveling.left-off.{}", self.project.get_value())
    }
}

fn shelf() -> Option<Storage> {
    window().local_storage().ok().flatten()
}
