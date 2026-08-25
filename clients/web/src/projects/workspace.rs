use leptos::prelude::*;

use crate::http::ApiError;
use crate::projects::model::{Project, ProjectId};
use crate::projects::service;

type ProjectListResource = LocalResource<Result<Vec<Project>, ApiError>>;
type CreateAction = Action<String, Result<Project, ApiError>>;
type RenameAction = Action<(ProjectId, String), Result<Project, ApiError>>;
type DeleteAction = Action<ProjectId, Result<(), ApiError>>;

#[derive(Clone, Copy)]
pub struct Workspace {
    project_list: ProjectListResource,
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
        let refetch_projects = Trigger::new();
        let problem = RwSignal::new(None);

        Self {
            project_list: LocalResource::new(move || {
                refetch_projects.track();
                async move { service::list().await }
            }),
            problem,
            creating: Action::new_local(move |name: &String| {
                let name = name.clone();
                async move { remember_outcome(problem, refetch_projects, service::create(&name).await) }
            }),
            renaming: Action::new_local(move |(id, name): &(ProjectId, String)| {
                let (id, name) = (id.clone(), name.clone());
                async move {
                    remember_outcome(problem, refetch_projects, service::rename(&id, &name).await)
                }
            }),
            deleting: Action::new_local(move |id: &ProjectId| {
                let id = id.clone();
                async move { remember_outcome(problem, refetch_projects, service::delete(&id).await) }
            }),
        }
    }

    pub fn projects(self) -> Vec<Project> {
        self.project_list
            .get()
            .and_then(|listed| listed.ok())
            .unwrap_or_default()
    }

    pub fn problem(self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn loading(self) -> bool {
        self.project_list.get().is_none()
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

fn remember_outcome<T>(
    problem: RwSignal<Option<ApiError>>,
    refetch_projects: Trigger,
    outcome: Result<T, ApiError>,
) -> Result<T, ApiError> {
    match &outcome {
        Ok(_) => {
            problem.set(None);
            refetch_projects.notify();
        }
        Err(failure) => problem.set(Some(failure.clone())),
    }

    outcome
}
