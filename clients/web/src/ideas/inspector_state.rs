use leptos::prelude::*;

use crate::appearances::model::Place;
use crate::appearances::service as appearance_service;
use crate::http::ApiError;
use crate::ideas::model::{Idea, IdeaId};
use crate::ideas::service as idea_service;
use crate::outline::model::{Section, SectionId};
use crate::outline::service as outline_service;
use crate::route;
use crate::scenes::model::{Scene, SceneId};
use crate::scenes::service as scene_service;

#[derive(Clone, Copy)]
pub struct InspectorState {
    project: Memo<String>,
    problem: RwSignal<Option<ApiError>>,
    opened: RwSignal<Option<Idea>>,
    places: RwSignal<Vec<Place>>,
    sections: RwSignal<Vec<Section>>,
    scenes: RwSignal<Vec<Scene>>,
    retitling_idea: Action<String, ()>,
}

impl InspectorState {
    pub fn inspecting(project: Memo<String>, inspected: Signal<Option<IdeaId>>) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let opened = RwSignal::new(None::<Idea>);
        let places = RwSignal::new(Vec::<Place>::new());
        let sections = RwSignal::new(Vec::<Section>::new());
        let scenes = RwSignal::new(Vec::<Scene>::new());

        let opening_idea = Action::new_local(move |asked: &IdeaId| {
            let asked = asked.clone();

            async move {
                let found = match idea_service::get(&asked).await {
                    Ok(found) => found,
                    Err(failure) => return problem.set(Some(failure)),
                };
                let appearing = match appearance_service::of(&asked).await {
                    Ok(appearing) => appearing,
                    Err(failure) => return problem.set(Some(failure)),
                };
                if inspected.get_untracked().as_ref() != Some(&asked) {
                    return;
                }
                problem.set(None);
                places.set(appearing);
                opened.set(Some(found));
            }
        });

        let listing_places = Action::new_local(move |segment: &String| {
            let project = route::project_id(segment);

            async move {
                match outline_service::open(&project).await {
                    Ok(found) => sections.set(found.sections),
                    Err(failure) => return problem.set(Some(failure)),
                }
                match scene_service::in_project(&project).await {
                    Ok(found) => scenes.set(found),
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        let retitling_idea = Action::new_local(move |typed: &String| {
            let typed = typed.clone();
            let idea = opened.get_untracked().map(|idea| idea.id);

            async move {
                let Some(idea) = idea else {
                    return;
                };

                match idea_service::retitle(&idea, &typed).await {
                    Ok(retitled) => {
                        problem.set(None);
                        opened.set(Some(retitled));
                    }
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        Effect::new(move || match inspected.get() {
            Some(asked) => {
                opening_idea.dispatch(asked);
            }
            None => opened.set(None),
        });

        Effect::new(move || {
            listing_places.dispatch(project.get());
        });

        Self {
            project,
            problem,
            opened,
            places,
            sections,
            scenes,
            retitling_idea,
        }
    }

    pub fn project(&self) -> String {
        self.project.get()
    }

    pub fn problem(&self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn opened(&self) -> Option<Idea> {
        self.opened.get()
    }

    pub fn retitle(&self, typed: String) {
        self.retitling_idea.dispatch(typed);
    }

    pub fn adopt(&self, newer: Idea) {
        let older = self.opened.with_untracked(|held| {
            held.as_ref()
                .is_some_and(|held| held.id == newer.id && held.version < newer.version)
        });

        if older {
            self.opened.set(Some(newer));
        }
    }

    pub fn noting_sections(&self) -> Vec<(SectionId, String)> {
        let sections = self.sections.get();

        self.places
            .get()
            .into_iter()
            .filter_map(|place| match place {
                Place::Section(id) => Some(id),
                Place::Scene(_) => None,
            })
            .map(|id| {
                let named = sections
                    .iter()
                    .find(|section| section.id == id)
                    .map(|section| section.title.clone())
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| "Untitled".to_owned());

                (id, named)
            })
            .collect()
    }

    pub fn linking_scenes(&self) -> Vec<(SceneId, String)> {
        let scenes = self.scenes.get();

        self.places
            .get()
            .into_iter()
            .filter_map(|place| match place {
                Place::Scene(id) => Some(id),
                Place::Section(_) => None,
            })
            .map(|id| {
                let named = scenes
                    .iter()
                    .find(|scene| scene.id == id)
                    .map(Scene::shown_as)
                    .unwrap_or_else(|| "Untitled".to_owned());

                (id, named)
            })
            .collect()
    }
}
