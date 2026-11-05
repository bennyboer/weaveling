use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task;

use crate::ideas::followed;
use crate::ideas::model::{Awaited, Idea, IdeaId};
use crate::live::Subscription;
use crate::projects::model::ProjectId;
use crate::scenes::model::SceneId;
use crate::scenes::service as scenes;
use crate::writing::problems::Problems;
use crate::writing::scene_copies::SceneCopies;

#[derive(Clone, Copy)]
pub struct SceneIdeas {
    project: StoredValue<ProjectId>,
    scenes: SceneCopies,
    asked: StoredValue<HashSet<SceneId>>,
    ideas: RwSignal<Option<Vec<Idea>>>,
    asking: Action<Option<Awaited>, ()>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
    picking: RwSignal<bool>,
    problems: Problems,
}

impl SceneIdeas {
    pub fn of(project: &ProjectId, scenes: SceneCopies, problems: Problems) -> Self {
        let ideas = RwSignal::new(None);
        let following = StoredValue::new_local(None::<Subscription>);

        Self {
            project: StoredValue::new(project.clone()),
            scenes,
            asked: StoredValue::new(HashSet::new()),
            ideas,
            asking: followed::asking(project, ideas, move |failure| problems.report(failure)),
            following,
            picking: RwSignal::new(false),
            problems,
        }
    }

    pub fn linked_to(self, scene: &SceneId) -> Option<Vec<IdeaId>> {
        self.scenes.ideas_of(scene)
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

    pub fn every_idea(self) -> Vec<Idea> {
        self.ideas.get().unwrap_or_default()
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

        task::spawn_local(async move {
            match scenes::open(&scene).await {
                Ok(found) => self.scenes.hold(found),
                Err(failure) => {
                    _ = self.asked.try_update_value(|asked| asked.remove(&scene));
                    self.problems.report(failure);
                }
            }
        });
    }

    pub fn know_the_ideas(self) {
        if self.following.with_value(Option::is_none) {
            let project = self.project.get_value();
            self.following
                .set_value(Some(followed::following(&project, self.ideas, self.asking)));
        }
    }

    pub fn link(self, scene: SceneId, chosen: Vec<IdeaId>) {
        let linked = self.linked_to(&scene).unwrap_or_default();
        let new: Vec<IdeaId> = chosen
            .into_iter()
            .filter(|idea| !linked.contains(idea))
            .collect();
        let linking = self.scenes.linked(scene, new);

        task::spawn_local(async move {
            linking.await;
        });
    }

    pub fn unlink(self, scene: SceneId, idea: IdeaId) {
        let unlinking = self.scenes.unlinked(scene, idea);

        task::spawn_local(async move {
            unlinking.await;
        });
    }
}
