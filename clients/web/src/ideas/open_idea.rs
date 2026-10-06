use leptos::prelude::*;

use crate::appearances::model::Place;
use crate::appearances::service as appearance_service;
use crate::http::ApiError;
use crate::ideas::model::Idea;
use crate::ideas::service as idea_service;
use crate::outline::model::{Section, SectionId};
use crate::outline::service as outline_service;
use crate::passages::model::{Passage, PassageId};
use crate::passages::service as passage_service;
use crate::route;

#[derive(Clone, Copy)]
pub struct OpenIdea {
    project: Memo<String>,
    problem: RwSignal<Option<ApiError>>,
    opened: RwSignal<Option<Idea>>,
    places: RwSignal<Vec<Place>>,
    sections: RwSignal<Vec<Section>>,
    passages: RwSignal<Vec<Passage>>,
    retitling_idea: Action<String, ()>,
}

impl OpenIdea {
    pub fn open(project: Memo<String>, asked: Memo<Option<String>>) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let opened = RwSignal::new(None::<Idea>);
        let places = RwSignal::new(Vec::<Place>::new());
        let sections = RwSignal::new(Vec::<Section>::new());
        let passages = RwSignal::new(Vec::<Passage>::new());

        let opening_idea = Action::new_local(move |asked: &String| {
            let asked = route::idea_id(asked);

            async move {
                let found = match idea_service::get(&asked).await {
                    Ok(found) => found,
                    Err(failure) => return problem.set(Some(failure)),
                };
                match appearance_service::of(&found.id).await {
                    Ok(found) => places.set(found),
                    Err(failure) => return problem.set(Some(failure)),
                }
                problem.set(None);
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
                match passage_service::in_project(&project).await {
                    Ok(found) => passages.set(found),
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

        Effect::new(move || {
            if let Some(asked) = asked.get() {
                opening_idea.dispatch(asked);
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
            passages,
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

    pub fn noting_sections(&self) -> Vec<(SectionId, String)> {
        let sections = self.sections.get();

        self.places
            .get()
            .into_iter()
            .filter_map(|place| match place {
                Place::Section(id) => Some(id),
                Place::Passage(_) => None,
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

    pub fn linking_passages(&self) -> Vec<(PassageId, String)> {
        let passages = self.passages.get();

        self.places
            .get()
            .into_iter()
            .filter_map(|place| match place {
                Place::Passage(id) => Some(id),
                Place::Section(_) => None,
            })
            .map(|id| {
                let named = passages
                    .iter()
                    .find(|passage| passage.id == id)
                    .map(Passage::shown_as)
                    .unwrap_or_else(|| "Untitled".to_owned());

                (id, named)
            })
            .collect()
    }
}
