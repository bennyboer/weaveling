use leptos::prelude::*;

use crate::feedback::model::Pending;
use crate::feedback::service;
use crate::outline::model::SectionId;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;

/// How much feedback given is not done yet in the project being written, on each scene and on each section itself,
/// asked for once the writing view opens and again whenever a piece is marked done or not done there.
#[derive(Clone, Copy)]
pub struct PendingHere {
    project: StoredValue<ProjectId>,
    counted: RwSignal<Pending>,
}

impl PendingHere {
    pub fn asked(project: ProjectId) -> Self {
        let here = Self {
            project: StoredValue::new(project),
            counted: RwSignal::new(Pending::default()),
        };
        here.again();

        here
    }

    /// Counts again, as a piece was marked done or not done.
    pub fn again(self) {
        let project = self.project.get_value();
        leptos::task::spawn_local(async move {
            if let Ok(counted) = service::pending_in(&project).await {
                _ = self.counted.try_set(counted);
            }
        });
    }

    /// The comments not done yet on a scene, on it or a passage in it.
    pub fn on_scene(self, scene: &SceneId) -> u32 {
        self.counted
            .with(|counted| counted.scenes.get(scene).copied().unwrap_or(0))
    }

    /// The comments not done yet on a section itself.
    pub fn on_section(self, section: &SectionId) -> u32 {
        self.counted
            .with(|counted| counted.sections.get(section).copied().unwrap_or(0))
    }
}
