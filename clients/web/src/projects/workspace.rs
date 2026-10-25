use std::time::Duration;

use leptos::prelude::*;
use leptos::task;

use crate::http::{ApiError, Refusal};
use crate::live::Following;
use crate::projects::model;
use crate::projects::model::{Membership, Project, ProjectDetails, ProjectId};
use crate::projects::service;
use live_contract::FrameDTO;

const NOT_YET_OPEN: &str = "project-not-yet-open";

type CreateAction = Action<(String, ProjectDetails), Result<Project, ApiError>>;
type RenameAction = Action<(ProjectId, String), Result<Project, ApiError>>;
type DeleteAction = Action<ProjectId, Result<(), ApiError>>;

#[derive(Clone, Debug)]
enum Opening {
    Idle,
    SettingUp,
    Opened(Project),
    Unopened(ApiError),
}

#[derive(Clone, Copy)]
pub struct Workspace {
    known: RwSignal<Option<Vec<Project>>>,
    problem: RwSignal<Option<ApiError>>,
    opening: RwSignal<Opening>,
    creating: CreateAction,
    renaming: RenameAction,
    deleting: DeleteAction,
}

impl Workspace {
    pub fn listing(with_accounts: bool) -> Self {
        let known = RwSignal::new(None::<Vec<Project>>);
        let problem = RwSignal::new(None);
        let opening = RwSignal::new(Opening::Idle);

        let latest = StoredValue::new(0_u64);
        let listing = Action::new_local(move |_: &()| {
            let asked = latest.get_value() + 1;
            latest.set_value(asked);

            async move {
                let answer = service::list().await;
                if latest.try_get_value() != Some(asked) {
                    return;
                }
                match answer {
                    Ok(listed) => _ = known.try_set(Some(listed)),
                    Err(failure) => {
                        known.update(|held| {
                            held.get_or_insert_with(Vec::new);
                        });
                        problem.set(Some(failure));
                    }
                }
            }
        });
        StoredValue::new_local(Following::start(
            "/api/projects/live",
            move |told: FrameDTO| {
                let Some(held) = known.try_get_untracked() else {
                    return;
                };
                match held.and_then(|held| model::taken_in(held, told)) {
                    Some(next) => _ = known.try_set(Some(next)),
                    None => _ = listing.dispatch(()),
                }
            },
            move || _ = listing.dispatch(()),
        ));
        if with_accounts {
            let reconnected = StoredValue::new(false);
            StoredValue::new_local(Following::start(
                "/api/members/live",
                move |told: FrameDTO| match model::membership_in(told) {
                    Some(Membership::Begun(project)) => {
                        take_in_joined(known, project, Duration::ZERO)
                    }
                    Some(Membership::Ended(project)) => {
                        _ = known.try_update(|held| {
                            if let Some(held) = held {
                                held.retain(|kept| kept.id != project);
                            }
                        })
                    }
                    None => {}
                },
                move || {
                    if reconnected.get_value() {
                        listing.dispatch(());
                    }
                    reconnected.set_value(true);
                },
            ));
        }

        Self {
            known,
            problem,
            opening,
            creating: Action::new_local(move |(name, details): &(String, ProjectDetails)| {
                let (name, details) = (name.clone(), details.clone());
                opening.set(Opening::Idle);
                async move {
                    let outcome = service::create(&name, &details).await;
                    if let Ok(created) = &outcome {
                        keep(known, |held| put(held, created.clone()));
                        opening.set(Opening::SettingUp);
                        open_once_it_answers(opening, created.clone(), Duration::ZERO);
                    }

                    outcome
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
        let (creating, opening) = (self.creating, self.opening);

        Signal::derive(move || {
            creating.pending().get() || matches!(opening.get(), Opening::SettingUp)
        })
    }

    pub fn setting_up(self) -> Signal<bool> {
        let opening = self.opening;

        Signal::derive(move || matches!(opening.get(), Opening::SettingUp))
    }

    pub fn created(self) -> Signal<Option<Project>> {
        let opening = self.opening;

        Signal::derive(move || match opening.get() {
            Opening::Opened(project) => Some(project),
            _ => None,
        })
    }

    pub fn refused_creation(self) -> Signal<Option<ApiError>> {
        let (creating, opening) = (self.creating, self.opening);

        Signal::derive(move || match opening.get() {
            Opening::Unopened(failure) => Some(failure),
            _ => creating.value().get().and_then(Result::err),
        })
    }

    pub fn create(self, name: String, details: ProjectDetails) {
        self.creating.dispatch((name, details));
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

fn open_once_it_answers(opening: RwSignal<Opening>, project: Project, waited: Duration) {
    task::spawn_local(async move {
        let next = match service::one(&project.id).await {
            Ok(_) => Opening::Opened(project),
            Err(ApiError::NotFound(_)) => match model::next_wait(waited) {
                Some(wait) => {
                    _ = set_timeout(
                        move || open_once_it_answers(opening, project, waited + wait),
                        wait,
                    );
                    return;
                }
                None => Opening::Unopened(ApiError::Rejected(Refusal {
                    kind: Some(NOT_YET_OPEN.to_owned()),
                    detail: String::new(),
                    particulars: Vec::new(),
                })),
            },
            Err(failure) => Opening::Unopened(failure),
        };
        _ = opening.try_set(next);
    });
}

fn take_in_joined(known: RwSignal<Option<Vec<Project>>>, project: ProjectId, waited: Duration) {
    task::spawn_local(async move {
        match service::one(&project).await {
            Ok(found) => {
                _ = known.try_update(|held| {
                    if let Some(held) = held {
                        put(held, found);
                    }
                })
            }
            Err(ApiError::NotFound(_)) => {
                if let Some(wait) = model::next_wait(waited) {
                    _ = set_timeout(move || take_in_joined(known, project, waited + wait), wait);
                }
            }
            Err(_) => {}
        }
    });
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
        Some(known) if known.version < project.version => *known = project,
        Some(_) => {}
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
