use std::collections::{HashMap, HashSet};
use std::time::Duration;

use feedback_contract::status;
use leptos::prelude::*;

use crate::feedback::followed::FollowedFeedback;
use crate::feedback::model::{Pending, PendingPassage};
use crate::feedback::service;
use crate::feedback::service::Asked;
use crate::outline::model::SectionId;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;

/// How long scenes coming into sight are gathered before their passages are asked for, so a scroll asks once.
const GATHERED_FOR: Duration = Duration::from_millis(100);

/// The feedback given and pending, not done yet, in the project being written: how much on each scene and on each section itself,
/// for the whole book, and the passages it is about in the scenes drawn now alone, so a long book with much feedback
/// asks only for what is in sight. Each scene drawn is watched until it is let go of; the passages of scenes coming
/// into sight are asked for together. All of it is asked for again whenever a piece is marked done or not done there,
/// and whenever the feedback given in the project changes, as it is followed live.
#[derive(Clone, Copy)]
pub struct PendingHere {
    project: StoredValue<ProjectId>,
    counted: RwSignal<Pending>,
    passages: RwSignal<HashMap<SceneId, Vec<PendingPassage>>>,
    watched: StoredValue<HashSet<SceneId>>,
    gathered: StoredValue<HashSet<SceneId>>,
    gathering: StoredValue<Option<TimeoutHandle>, LocalStorage>,
}

impl PendingHere {
    pub fn asked(project: ProjectId) -> Self {
        let here = Self {
            project: StoredValue::new(project),
            counted: RwSignal::new(Pending::default()),
            passages: RwSignal::new(HashMap::new()),
            watched: StoredValue::new(HashSet::new()),
            gathered: StoredValue::new(HashSet::new()),
            gathering: StoredValue::new_local(None),
        };
        here.counted_again();
        let followed = expect_context::<FollowedFeedback>();
        Effect::new(move |heard_before: Option<()>| {
            followed.heard();
            if heard_before.is_some() {
                here.again();
            }
        });

        here
    }

    /// Asks again for the counts and the passages in sight, as a piece was marked done or not done.
    pub fn again(self) {
        self.counted_again();
        let watched = self
            .watched
            .with_value(|watched| watched.iter().cloned().collect());
        self.asked_for(watched);
    }

    fn counted_again(self) {
        let project = self.project.get_value();
        leptos::task::spawn_local(async move {
            if let Ok(counted) = service::pending(&Asked::in_project(&project)).await {
                _ = self.counted.try_set(counted);
            }
        });
    }

    /// A scene came into sight: its passages are asked for, with those of the others coming into sight about now.
    pub fn watch(self, scene: SceneId) {
        self.watched.update_value(|watched| {
            watched.insert(scene.clone());
        });
        self.gathered.update_value(|gathered| {
            gathered.insert(scene);
        });
        if self.gathering.with_value(Option::is_some) {
            return;
        }
        let asked = move || {
            _ = self.gathering.try_set_value(None);
            let gathered = self
                .gathered
                .try_update_value(|gathered| gathered.drain().collect::<Vec<_>>())
                .unwrap_or_default();
            self.asked_for(gathered);
        };
        self.gathering
            .set_value(set_timeout(asked, GATHERED_FOR).ok());
    }

    /// A scene went out of sight: its passages are let go of.
    pub fn let_go_of(self, scene: &SceneId) {
        _ = self
            .watched
            .try_update_value(|watched| watched.remove(scene));
        _ = self.passages.try_update(|passages| passages.remove(scene));
    }

    fn asked_for(self, scenes: Vec<SceneId>) {
        if scenes.is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            let asked = Asked {
                scenes: scenes.clone(),
                ..Asked::in_project(&self.project.get_value())
            };
            let Ok(given) = service::submitted(&asked, Some(status::PENDING)).await else {
                return;
            };
            let found = given.iter().filter_map(PendingPassage::of);
            _ = self.passages.try_update(|passages| {
                for scene in &scenes {
                    passages.remove(scene);
                }
                for passage in found {
                    let still_watched = self
                        .watched
                        .with_value(|watched| watched.contains(&passage.scene));
                    if still_watched {
                        passages
                            .entry(passage.scene.clone())
                            .or_default()
                            .push(passage);
                    }
                }
            });
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

    /// The passages comments not done yet are about, in the scenes in sight.
    pub fn passages(self) -> Vec<PendingPassage> {
        self.passages
            .with(|passages| passages.values().flatten().cloned().collect())
    }
}
