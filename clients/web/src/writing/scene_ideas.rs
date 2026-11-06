use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task;

use crate::ideas::followed::FollowedIdeas;
use crate::ideas::model::{Idea, IdeaId};
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
    ideas: RwSignal<Option<FollowedIdeas>>,
    owner: StoredValue<Owner, LocalStorage>,
    picking: RwSignal<bool>,
    problems: Problems,
}

impl SceneIdeas {
    pub fn of(project: &ProjectId, scenes: SceneCopies, problems: Problems) -> Self {
        Self {
            project: StoredValue::new(project.clone()),
            scenes,
            asked: StoredValue::new(HashSet::new()),
            ideas: RwSignal::new(None),
            owner: StoredValue::new_local(Owner::current().unwrap_or_default()),
            picking: RwSignal::new(false),
            problems,
        }
    }

    pub fn linked_to(self, scene: &SceneId) -> Option<Vec<IdeaId>> {
        self.scenes.ideas_of(scene)
    }

    pub fn named(self, idea: &IdeaId) -> String {
        self.ideas
            .get()
            .and_then(|ideas| ideas.copies().shown_one(idea))
            .map(|held| held.shown_as())
            .unwrap_or_default()
    }

    pub fn every_idea(self) -> Vec<Idea> {
        self.ideas
            .get()
            .map(|ideas| ideas.copies().shown())
            .unwrap_or_default()
    }

    pub fn waiting_for(self, scene: &SceneId) -> Vec<Idea> {
        let linked = self.linked_to(scene).unwrap_or_default();

        self.every_idea()
            .into_iter()
            .filter(|idea| !linked.contains(&idea.id))
            .collect()
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
        if self.ideas.with_untracked(Option::is_some) {
            return;
        }
        let (project, owner) = (self.project.get_value(), self.owner.get_value());
        let problems = self.problems;
        let followed =
            owner.with(|| FollowedIdeas::follow(&project, move |failure| problems.report(failure)));
        self.ideas.set(Some(followed));
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
