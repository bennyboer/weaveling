use leptos::prelude::*;

use crate::http::ApiError;
use crate::projects::model::{Project, ProjectId};
use crate::projects::service;

type CreateAction = Action<String, Result<Project, ApiError>>;
type RenameAction = Action<(ProjectId, String), Result<Project, ApiError>>;
type DeleteAction = Action<ProjectId, Result<(), ApiError>>;

#[derive(Clone, Copy)]
pub struct Workspace {
    known: RwSignal<Option<Vec<Project>>>,
    problem: RwSignal<Option<ApiError>>,
    creating: CreateAction,
    renaming: RenameAction,
    deleting: DeleteAction,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        let known = RwSignal::new(None::<Vec<Project>>);
        let problem = RwSignal::new(None);

        let listing = Action::new_local(move |_: &()| async move {
            match service::list().await {
                Ok(mut listed) => known.update(|held| {
                    for written in held.take().unwrap_or_default() {
                        put(&mut listed, written);
                    }
                    *held = Some(listed);
                }),
                Err(failure) => {
                    known.update(|held| {
                        held.get_or_insert_with(Vec::new);
                    });
                    problem.set(Some(failure));
                }
            }
        });
        listing.dispatch(());

        Self {
            known,
            problem,
            creating: Action::new_local(move |name: &String| {
                let name = name.clone();
                async move {
                    let outcome = service::create(&name).await;
                    if let Ok(created) = &outcome {
                        keep(known, |held| put(held, created.clone()));
                    }

                    remember_outcome(problem, outcome)
                }
            }),
            renaming: Action::new_local(move |(id, name): &(ProjectId, String)| {
                let (id, name) = (id.clone(), name.clone());
                async move {
                    let outcome = service::rename(&id, &name).await;
                    if let Ok(renamed) = &outcome {
                        keep(known, |held| put(held, renamed.clone()));
                    }

                    remember_outcome(problem, outcome)
                }
            }),
            deleting: Action::new_local(move |id: &ProjectId| {
                let id = id.clone();
                async move {
                    let outcome = service::delete(&id).await;
                    if outcome.is_ok() {
                        keep(known, |held| held.retain(|project| project.id != id));
                    }

                    remember_outcome(problem, outcome)
                }
            }),
        }
    }

    pub fn projects(self) -> Vec<Project> {
        self.known.get().unwrap_or_default()
    }

    pub fn problem(self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn loading(self) -> bool {
        self.known.get().is_none()
    }

    pub fn creating(self) -> Signal<bool> {
        pending(self.creating)
    }

    pub fn created(self) -> Signal<Option<Project>> {
        let creating = self.creating;

        Signal::derive(move || creating.value().get().and_then(Result::ok))
    }

    pub fn create(self, name: String) {
        self.creating.dispatch(name);
    }

    pub fn renaming(self) -> Signal<bool> {
        pending(self.renaming)
    }

    pub fn rename(self, id: ProjectId, name: String) {
        self.renaming.dispatch((id, name));
    }

    pub fn deleting(self) -> Signal<bool> {
        pending(self.deleting)
    }

    pub fn delete(self, id: ProjectId) {
        self.deleting.dispatch(id);
    }
}

fn pending<I, O>(action: Action<I, O>) -> Signal<bool>
where
    I: 'static,
    O: 'static,
{
    Signal::derive(move || action.pending().get())
}

fn keep(known: RwSignal<Option<Vec<Project>>>, change: impl FnOnce(&mut Vec<Project>)) {
    known.update(|held| change(held.get_or_insert_with(Vec::new)));
}

fn put(held: &mut Vec<Project>, project: Project) {
    match held.iter_mut().find(|known| known.id == project.id) {
        Some(known) => *known = project,
        None => held.push(project),
    }
    held.sort_by(|one, other| other.id.cmp(&one.id));
}

fn remember_outcome<T>(
    problem: RwSignal<Option<ApiError>>,
    outcome: Result<T, ApiError>,
) -> Result<T, ApiError> {
    match &outcome {
        Ok(_) => problem.set(None),
        Err(failure) => problem.set(Some(failure.clone())),
    }

    outcome
}
