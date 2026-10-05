use leptos::prelude::*;

use crate::http::ApiError;
use crate::ideas::model::{Idea, IdeaId};
use crate::ideas::service as idea_service;
use crate::passages::model::PassageId;
use crate::passages::service as passage_service;
use crate::route;

#[derive(Clone, Copy)]
pub struct OpenPassage {
    project: Memo<String>,
    problem: RwSignal<Option<ApiError>>,
    opened: RwSignal<Option<PassageId>>,
    title: RwSignal<String>,
    ideas: RwSignal<Vec<Idea>>,
    linked_ideas: RwSignal<Vec<IdeaId>>,
    picking: RwSignal<bool>,
    retitling_passage: Action<String, ()>,
    linking_ideas: Action<Vec<IdeaId>, ()>,
    unlinking_idea: Action<IdeaId, ()>,
}

impl OpenPassage {
    pub fn open(project: Memo<String>, asked: Memo<Option<String>>) -> Self {
        let problem = RwSignal::new(None::<ApiError>);
        let opened = RwSignal::new(None::<PassageId>);
        let title = RwSignal::new(String::new());
        let ideas = RwSignal::new(Vec::<Idea>::new());
        let linked_ideas = RwSignal::new(Vec::<IdeaId>::new());
        let picking = RwSignal::new(false);

        let opening_passage = Action::new_local(move |asked: &PassageId| {
            let asked = asked.clone();

            async move {
                match passage_service::open(&asked).await {
                    Ok(found) => {
                        problem.set(None);
                        title.set(found.title);
                        linked_ideas.set(found.ideas);
                        opened.set(Some(found.id));
                    }
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        let listing_ideas = Action::new_local(move |segment: &String| {
            let project = route::project_id(segment);

            async move {
                match idea_service::list(&project).await {
                    Ok(found) => ideas.set(found),
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        let retitling_passage = Action::new_local(move |typed: &String| {
            let typed = typed.clone();
            let passage = opened.get_untracked();

            async move {
                let Some(passage) = passage else {
                    return;
                };

                match passage_service::retitle(&passage, &typed).await {
                    Ok(retitled) => {
                        problem.set(None);
                        title.set(retitled.title);
                    }
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        let linking_ideas = Action::new_local(move |chosen: &Vec<IdeaId>| {
            let chosen = chosen.clone();
            let passage = opened.get_untracked();

            async move {
                let Some(passage) = passage else {
                    return;
                };

                for idea in &chosen {
                    match passage_service::link(&passage, idea).await {
                        Ok(updated) => linked_ideas.set(updated.ideas),
                        Err(failure) => return problem.set(Some(failure)),
                    }
                }
                problem.set(None);
            }
        });

        let unlinking_idea = Action::new_local(move |idea: &IdeaId| {
            let idea = idea.clone();
            let passage = opened.get_untracked();

            async move {
                let Some(passage) = passage else {
                    return;
                };

                match passage_service::unlink(&passage, &idea).await {
                    Ok(updated) => {
                        problem.set(None);
                        linked_ideas.set(updated.ideas);
                    }
                    Err(failure) => problem.set(Some(failure)),
                }
            }
        });

        Effect::new(move || {
            if let Some(asked) = asked.get() {
                opening_passage.dispatch(PassageId::from(asked));
            }
        });

        Effect::new(move || {
            listing_ideas.dispatch(project.get());
        });

        Self {
            project,
            problem,
            opened,
            title,
            ideas,
            linked_ideas,
            picking,
            retitling_passage,
            linking_ideas,
            unlinking_idea,
        }
    }

    pub fn project(&self) -> String {
        self.project.get()
    }

    pub fn problem(&self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn opened(&self) -> Option<PassageId> {
        self.opened.get()
    }

    pub fn title(&self) -> String {
        self.title.get()
    }

    pub fn retitle(&self, typed: String) {
        self.retitling_passage.dispatch(typed);
    }

    pub fn linked_ideas(&self) -> Vec<IdeaId> {
        self.linked_ideas.get()
    }

    pub fn waiting_ideas(&self) -> Vec<Idea> {
        let linked_ideas = self.linked_ideas.get();

        self.ideas
            .get()
            .into_iter()
            .filter(|idea| !linked_ideas.contains(&idea.id))
            .collect()
    }

    pub fn named(&self, idea: &IdeaId) -> String {
        self.ideas
            .with(|known| {
                known
                    .iter()
                    .find(|held| &held.id == idea)
                    .map(|held| held.shown_as().to_owned())
            })
            .unwrap_or_else(|| "Untitled".to_owned())
    }

    pub fn link(&self, chosen: Vec<IdeaId>) {
        self.linking_ideas.dispatch(chosen);
    }

    pub fn unlink(&self, idea: IdeaId) {
        self.unlinking_idea.dispatch(idea);
    }

    pub fn is_picking(&self) -> bool {
        self.picking.get()
    }

    pub fn start_picking(&self) {
        self.picking.set(true);
    }

    pub fn stop_picking(&self) {
        self.picking.set(false);
    }
}
