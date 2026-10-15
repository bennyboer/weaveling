use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::ideas::model::IdeaId;
use crate::ideas::service as ideas;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;
use crate::writing::saving::Saving;

#[derive(Clone, Copy)]
pub struct SceneIdeas {
    project: StoredValue<ProjectId>,
    linked: RwSignal<HashMap<SceneId, Vec<IdeaId>>>,
    asked: StoredValue<HashSet<SceneId>>,
    names: RwSignal<Option<HashMap<IdeaId, String>>>,
    names_asked: StoredValue<bool>,
    problems: Problems,
    saving: Saving,
}

impl SceneIdeas {
    pub fn of(project: &ProjectId, problems: Problems, saving: Saving) -> Self {
        Self {
            project: StoredValue::new(project.clone()),
            linked: RwSignal::new(HashMap::new()),
            asked: StoredValue::new(HashSet::new()),
            names: RwSignal::new(None),
            names_asked: StoredValue::new(false),
            problems,
            saving,
        }
    }

    pub fn linked_to(self, scene: &SceneId) -> Option<Vec<IdeaId>> {
        self.linked.with(|linked| linked.get(scene).cloned())
    }

    pub fn named(self, idea: &IdeaId) -> String {
        self.names.with(|names| {
            names
                .as_ref()
                .and_then(|names| names.get(idea).cloned())
                .unwrap_or_default()
        })
    }

    pub fn look_up(self, scene: SceneId) {
        if !self.names_asked.get_value() {
            self.names_asked.set_value(true);
            let project = self.project.get_value();

            spawn_local(async move {
                match ideas::list(&project).await {
                    Ok(found) => self.names.set(Some(
                        found
                            .iter()
                            .map(|idea| (idea.id.clone(), idea.shown_as().to_owned()))
                            .collect(),
                    )),
                    Err(failure) => self.problems.report(failure),
                }
            });
        }

        if !self
            .asked
            .try_update_value(|asked| asked.insert(scene.clone()))
            .unwrap_or(false)
        {
            return;
        }

        spawn_local(async move {
            match scenes::open(&scene).await {
                Ok(found) => self
                    .linked
                    .update(|linked| _ = linked.insert(scene, found.ideas)),
                Err(failure) => {
                    _ = self.asked.try_update_value(|asked| asked.remove(&scene));
                    self.problems.report(failure);
                }
            }
        });
    }

    pub fn unlink(self, scene: SceneId, idea: IdeaId) {
        let before = self.linked_to(&scene);
        self.linked.update(|linked| {
            if let Some(ideas) = linked.get_mut(&scene) {
                ideas.retain(|held| held != &idea);
            }
        });
        self.saving.write_begun();

        spawn_local(async move {
            match scenes::unlink(&scene, &idea).await {
                Ok(found) => self
                    .linked
                    .update(|linked| _ = linked.insert(scene, found.ideas)),
                Err(failure) => {
                    if let Some(before) = before {
                        self.linked
                            .update(|linked| _ = linked.insert(scene, before));
                    }
                    self.problems.report(failure);
                }
            }
            self.saving.write_done();
        });
    }
}
