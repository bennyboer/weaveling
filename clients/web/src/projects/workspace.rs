use std::time::Duration;

use leptos::prelude::*;
use leptos::task;

use crate::act::{Act, Here};
use crate::asking;
use crate::asking::Asking;
use crate::http::{ApiError, Refusal};
use crate::live::Subscription;
use crate::projects::model;
use crate::projects::model::{
    Awaited, AwaitedInTheTrash, Membership, Project, ProjectDetails, ProjectId, TrashedProject,
    Version,
};
use crate::projects::service;
use live_contract::FrameDTO;

const NOT_YET_OPEN: &str = "project-not-yet-open";

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
    creating: Act<(String, ProjectDetails)>,
    creation_refused: RwSignal<Option<ApiError>>,
    renaming: Act<(ProjectId, String)>,
    trashing: Act<ProjectId>,
    trashed: RwSignal<Option<Vec<TrashedProject>>>,
    restoring: Act<ProjectId>,
}

impl Workspace {
    pub fn listing(with_accounts: bool) -> Self {
        let known = RwSignal::new(None::<Vec<Project>>);
        let problem = RwSignal::new(None);
        let opening = RwSignal::new(Opening::Idle);

        let asking_the_list = Asking::new();
        let listing = Action::new_local(move |awaited: &Option<Awaited>| {
            let asked = asking_the_list.next();
            let awaited = awaited.clone();

            async move {
                let answer = asking::until_it_holds(
                    service::list,
                    |answer: &Result<Vec<Project>, ApiError>| match (answer, &awaited) {
                        (Ok(listed), Some(awaited)) => awaited.holds(listed),
                        _ => true,
                    },
                    asking::paused,
                )
                .await;
                if !asking_the_list.is_latest(asked) {
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
        let trashed = RwSignal::new(None::<Vec<TrashedProject>>);
        let trash_asking = Asking::new();
        let asking_the_trash = Action::new_local(move |awaited: &Option<AwaitedInTheTrash>| {
            let asked = trash_asking.next();
            let awaited = awaited.clone();

            async move {
                let answer = asking::until_it_holds(
                    service::in_the_trash,
                    |answer: &Result<Vec<TrashedProject>, ApiError>| match (answer, &awaited) {
                        (Ok(trashed), Some(awaited)) => awaited.holds(trashed),
                        _ => true,
                    },
                    asking::paused,
                )
                .await;
                if !trash_asking.is_latest(asked) {
                    return;
                }
                match answer {
                    Ok(found) => _ = trashed.try_set(Some(found)),
                    Err(failure) => _ = problem.try_set(Some(failure)),
                }
            }
        });
        asking_the_trash.dispatch(None);
        StoredValue::new_local(Subscription::start(
            "projects",
            &[("mine", "true")],
            move |told: FrameDTO| {
                if let Some(awaited) = AwaitedInTheTrash::told(&told) {
                    asking_the_trash.dispatch(Some(awaited));
                }
                let Some(held) = known.try_get_untracked() else {
                    return;
                };
                let awaited = Awaited::told(&told);
                match held.and_then(|held| model::taken_in(held, told)) {
                    Some(next) => _ = known.try_set(Some(next)),
                    None => _ = listing.dispatch(awaited),
                }
            },
            move || _ = listing.dispatch(None),
        ));
        if with_accounts {
            let reconnected = StoredValue::new(false);
            StoredValue::new_local(Subscription::start(
                "members",
                &[("mine", "true")],
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
                        listing.dispatch(None);
                    }
                    reconnected.set_value(true);
                },
            ));
        }

        let creation_refused = RwSignal::new(None);

        Self {
            known,
            problem,
            opening,
            creating: Act::new(move |(name, details): (String, ProjectDetails), _: Here| {
                opening.set(Opening::Idle);
                async move {
                    let outcome = service::create(&name, &details).await;
                    if let Ok(created) = &outcome {
                        keep(known, |held| put(held, created.clone()));
                        opening.set(Opening::SettingUp);
                        open_once_it_answers(opening, created.clone(), Duration::ZERO);
                    }

                    _ = creation_refused.try_set(outcome.err());
                }
            }),
            creation_refused,
            renaming: Act::new(move |(id, name): (ProjectId, String), _: Here| async move {
                let outcome = service::rename(&id, &name).await;
                if let Ok(renamed) = &outcome {
                    keep(known, |held| put(held, renamed.clone()));
                }

                remember_outcome(problem, outcome);
            }),
            trashing: Act::new(move |id: ProjectId, _: Here| async move {
                let outcome = service::trash(&id).await;
                if outcome.is_ok() {
                    keep(known, |held| held.retain(|project| project.id != id));
                    asking_the_trash.dispatch(Some(AwaitedInTheTrash::In(id.clone())));
                }

                remember_outcome(problem, outcome);
            }),
            trashed,
            restoring: Act::new(move |id: ProjectId, _: Here| async move {
                let outcome = service::restore(&id).await;
                if outcome.is_ok() {
                    _ = trashed.try_update(|held| {
                        if let Some(held) = held {
                            held.retain(|project| project.id != id);
                        }
                    });
                    listing.dispatch(Some(Awaited::Listed {
                        id: id.clone(),
                        at_least: Version::from(0),
                    }));
                }

                remember_outcome(problem, outcome);
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

        Signal::derive(move || creating.running() || matches!(opening.get(), Opening::SettingUp))
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
        let (refused, opening) = (self.creation_refused, self.opening);

        Signal::derive(move || match opening.get() {
            Opening::Unopened(failure) => Some(failure),
            _ => refused.get(),
        })
    }

    pub fn create(self, name: String, details: ProjectDetails) {
        self.creating.start((name, details));
    }

    pub fn renaming(self) -> Option<&'static str> {
        self.renaming.busy()
    }

    pub fn rename(self, id: ProjectId, name: String) {
        self.renaming.start((id, name));
    }

    pub fn trashing(self) -> Option<&'static str> {
        self.trashing.busy()
    }

    pub fn trash(self, id: ProjectId) {
        self.trashing.start(id);
    }

    pub fn in_the_trash(self) -> Vec<TrashedProject> {
        self.trashed.get().unwrap_or_default()
    }

    pub fn trash_asked(self) -> bool {
        self.trashed.get().is_some()
    }

    pub fn restoring(self, id: &ProjectId) -> Option<&'static str> {
        self.restoring.busy_on(id)
    }

    pub fn restore(self, id: ProjectId) {
        self.restoring.start(id);
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

fn remember_outcome<T>(problem: RwSignal<Option<ApiError>>, outcome: Result<T, ApiError>) {
    problem.set(outcome.err());
}
