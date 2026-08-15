use leptos::prelude::*;
use leptos::task::spawn_local;
use projects_contract::ProjectDTO;

use crate::api::{self, ApiError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activity {
    Loading,
    Creating,
    Renaming(String),
    Deleting(String),
}

#[derive(Clone, Copy)]
pub struct Workspace {
    projects: RwSignal<Vec<ProjectDTO>>,
    problem: RwSignal<Option<ApiError>>,
    in_flight: RwSignal<Vec<Activity>>,
    busy: Memo<bool>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        let in_flight = RwSignal::new(Vec::new());

        Self {
            projects: RwSignal::new(Vec::new()),
            problem: RwSignal::new(None),
            in_flight,
            busy: Memo::new(move |_| in_flight.with(|running| !running.is_empty())),
        }
    }

    pub fn projects(self) -> ReadSignal<Vec<ProjectDTO>> {
        self.projects.read_only()
    }

    pub fn problem(self) -> ReadSignal<Option<ApiError>> {
        self.problem.read_only()
    }

    pub fn busy(self) -> Memo<bool> {
        self.busy
    }

    pub fn reload(self) {
        self.begin(Activity::Loading);
        spawn_local(async move {
            let listed = api::list().await;
            self.end(&Activity::Loading);

            match listed {
                Ok(found) => {
                    self.projects.set(found);
                    self.problem.set(None);
                }
                Err(failure) => self.failed(failure),
            }
        });
    }

    pub fn create(self, name: String, on_created: impl FnOnce() + 'static) {
        self.begin(Activity::Creating);
        spawn_local(async move {
            let created = api::create(&name).await;
            self.end(&Activity::Creating);

            match created {
                Ok(_) => {
                    on_created();
                    self.succeeded();
                }
                Err(failure) => self.failed(failure),
            }
        });
    }

    pub fn rename(self, id: String, name: String) {
        let activity = Activity::Renaming(id.clone());
        self.begin(activity.clone());
        spawn_local(async move {
            let renamed = api::rename(&id, &name).await;
            self.end(&activity);

            match renamed {
                Ok(_) => self.succeeded(),
                Err(failure) => self.failed(failure),
            }
        });
    }

    pub fn delete(self, id: String) {
        let activity = Activity::Deleting(id.clone());
        self.begin(activity.clone());
        spawn_local(async move {
            let deleted = api::delete(&id).await;
            self.end(&activity);

            match deleted {
                Ok(()) => self.succeeded(),
                Err(failure) => self.failed(failure),
            }
        });
    }

    fn begin(self, activity: Activity) {
        self.in_flight.update(|running| running.push(activity));
    }

    fn end(self, activity: &Activity) {
        self.in_flight.update(|running| {
            if let Some(at) = running.iter().position(|running| running == activity) {
                running.remove(at);
            }
        });
    }

    fn succeeded(self) {
        self.problem.set(None);
        self.reload();
    }

    fn failed(self, failure: ApiError) {
        self.problem.set(Some(failure));
    }
}
