use appearances_contract::IdeaPlacesDTO;
use leptos::prelude::*;

use crate::appearances::model::Place;
use crate::appearances::service as appearance_service;
use crate::http::ApiError;
use crate::ideas::model::{Idea, IdeaId};
use crate::ideas::service as idea_service;
use crate::live::Subscription;
use crate::outline::model::{Section, SectionId};
use crate::outline::service as outline_service;
use crate::route;
use crate::scenes::followed as scenes_followed;
use crate::scenes::model;
use crate::scenes::model::{SceneId, SceneSummary};
use crate::speech;
use live_contract::FrameDTO;

#[derive(Clone, Copy)]
pub struct InspectorState {
    project: Memo<String>,
    problem: RwSignal<Option<ApiError>>,
    opened: RwSignal<Option<Idea>>,
    places: RwSignal<Vec<Place>>,
    sections: RwSignal<Vec<Section>>,
    scenes: RwSignal<Vec<SceneSummary>>,
    retitling_idea: Action<String, ()>,
}

impl InspectorState {
    pub fn inspecting(project: Memo<String>, inspected: Signal<Option<IdeaId>>) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let opened = RwSignal::new(None::<Idea>);
        let places = RwSignal::new(Vec::<Place>::new());
        let sections = RwSignal::new(Vec::<Section>::new());
        let scenes = RwSignal::new(Vec::<SceneSummary>::new());

        let opening_idea = Action::new_local(move |asked: &IdeaId| {
            let asked = asked.clone();

            async move {
                let found = match idea_service::get(&asked).await {
                    Ok(found) => found,
                    Err(failure) => return problem.set(Some(failure)),
                };
                if inspected.get_untracked().as_ref() != Some(&asked) {
                    return;
                }
                problem.set(None);
                opened.set(Some(found));
            }
        });

        let told_since = StoredValue::new(0_u64);
        let asking_places = Action::new_local(move |asked: &IdeaId| {
            let asked = asked.clone();
            let asked_at = told_since.get_value();

            async move {
                let appearing = match appearance_service::of(&asked).await {
                    Ok(appearing) => appearing,
                    Err(failure) => return problem.set(Some(failure)),
                };
                let still_asked = inspected.get_untracked().as_ref() == Some(&asked);
                if still_asked && told_since.get_value() == asked_at {
                    places.set(appearing);
                }
            }
        });

        let listing_places = Action::new_local(move |segment: &String| {
            let project = route::project_id(segment);

            async move {
                match outline_service::open(&project).await {
                    Ok(found) => sections.set(found.sections),
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

        let following = StoredValue::new_local(None::<Subscription>);
        let scenes_followed = StoredValue::new_local(None::<(Subscription, Box<dyn Fn()>)>);
        let asking_scenes =
            move || scenes_followed.with_value(|held| held.as_ref().map(|(_, ask)| ask()));
        let heard = move |told: FrameDTO| {
            let Ok(found) = serde_json::from_value::<IdeaPlacesDTO>(told.event) else {
                return;
            };
            told_since.update_value(|since| *since += 1);
            let appearing: Vec<Place> = found.places.into_iter().map(Place::from).collect();
            let unknown = appearing.iter().any(|place| match place {
                Place::Section(id) => {
                    sections.with_untracked(|known| known.iter().all(|section| &section.id != id))
                }
                Place::Scene(id) => {
                    scenes.with_untracked(|known| known.iter().all(|scene| &scene.id != id))
                }
            });
            places.set(appearing);
            if unknown {
                listing_places.dispatch(project.get_untracked());
                asking_scenes();
            }
        };

        Effect::new(move || match inspected.get() {
            Some(asked) => {
                if scenes_followed.with_value(Option::is_none) {
                    let (followed, ask) = scenes_followed::follow(
                        &route::project_id(&project.get_untracked()),
                        move |listed| _ = scenes.try_set(listed.to_vec()),
                        move |summary| scenes.update(|held| model::hold(held, summary.clone())),
                        move |failure| _ = problem.try_set(Some(failure)),
                    );
                    scenes_followed.set_value(Some((followed, Box::new(ask))));
                }
                places.set(Vec::new());
                opening_idea.dispatch(asked.clone());
                following.set_value(Some(Subscription::start(
                    "appearances",
                    &[("idea", asked.to_string().as_str())],
                    heard,
                    move || _ = asking_places.dispatch(asked.clone()),
                )));
            }
            None => {
                opened.set(None);
                following.set_value(None);
            }
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
                    .unwrap_or_else(|| speech::say("untitled"));

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
                    .map(SceneSummary::shown_as)
                    .unwrap_or_else(|| speech::say("untitled"));

                (id, named)
            })
            .collect()
    }
}
