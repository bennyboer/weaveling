use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::ideas::model::{Idea, IdeaId};
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
    ideas: RwSignal<Option<Vec<Idea>>>,
    ideas_asked: StoredValue<bool>,
    picking: RwSignal<bool>,
    problems: Problems,
    saving: Saving,
}

impl SceneIdeas {
    pub fn of(project: &ProjectId, problems: Problems, saving: Saving) -> Self {
        Self {
            project: StoredValue::new(project.clone()),
            linked: RwSignal::new(HashMap::new()),
            asked: StoredValue::new(HashSet::new()),
            ideas: RwSignal::new(None),
            ideas_asked: StoredValue::new(false),
            picking: RwSignal::new(false),
            problems,
            saving,
        }
    }

    pub fn linked_to(self, scene: &SceneId) -> Option<Vec<IdeaId>> {
        self.linked.with(|linked| linked.get(scene).cloned())
    }

    pub fn named(self, idea: &IdeaId) -> String {
        self.ideas.with(|ideas| {
            ideas
                .iter()
                .flatten()
                .find(|held| &held.id == idea)
                .map(|held| held.shown_as().to_owned())
                .unwrap_or_default()
        })
    }

    pub fn waiting_for(self, scene: &SceneId) -> Vec<Idea> {
        let linked = self.linked_to(scene).unwrap_or_default();

        self.ideas.with(|ideas| {
            ideas
                .iter()
                .flatten()
                .filter(|idea| !linked.contains(&idea.id))
                .cloned()
                .collect()
        })
    }

    pub fn is_picking(self) -> bool {
        self.picking.get()
    }

    pub fn start_picking(self) {
        self.picking.set(true);
    }

    pub fn stop_picking(self) {
        self.picking.set(false);
    }

    pub fn look_up(self, scene: SceneId) {
        self.know_the_ideas();

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

    pub fn know_the_ideas(self) {
        if !self.ideas_asked.get_value() {
            self.ideas_asked.set_value(true);
            let project = self.project.get_value();

            spawn_local(async move {
                match ideas::list(&project).await {
                    Ok(found) => self.ideas.set(Some(found)),
                    Err(failure) => self.problems.report(failure),
                }
            });
        }
    }

    pub fn link(self, scene: SceneId, chosen: Vec<IdeaId>) {
        let before = self.linked_to(&scene);
        self.linked.update(|linked| {
            let ideas = linked.entry(scene.clone()).or_default();
            ideas.extend(
                chosen
                    .iter()
                    .filter(|idea| !ideas.contains(idea))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
        });
        self.saving.write_begun();

        spawn_local(async move {
            for idea in &chosen {
                match scenes::link(&scene, idea).await {
                    Ok(found) => self
                        .linked
                        .update(|linked| _ = linked.insert(scene.clone(), found.ideas)),
                    Err(failure) => {
                        if let Some(before) = before {
                            self.linked
                                .update(|linked| _ = linked.insert(scene.clone(), before));
                        }
                        self.problems.report(failure);
                        break;
                    }
                }
            }
            self.saving.write_done();
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
