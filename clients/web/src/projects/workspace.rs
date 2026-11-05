use std::time::Duration;

use leptos::prelude::*;
use leptos::task;

use crate::act::{Act, Here};
use crate::asking;
use crate::asking::Asking;
use crate::http::{ApiError, Refusal};
use crate::live::Subscription;
use crate::projects::intent::Intent;
use crate::projects::model;
use crate::projects::model::{
    Awaited, AwaitedInTheTrash, Membership, Project, ProjectDetails, ProjectId, TrashedProject,
    Version,
};
use crate::projects::service;
use crate::working_copy::WorkingCopies;
use live_contract::FrameDTO;

const NOT_YET_OPEN: &str = "project-not-yet-open";

type Listed = WorkingCopies<ProjectId, Project, Intent>;
type Trashed = WorkingCopies<ProjectId, TrashedProject, Intent>;

#[derive(Clone, Debug)]
enum Opening {
    Idle,
    SettingUp,
    Opened(Project),
    Unopened(ApiError),
}

#[derive(Clone, Copy)]
pub struct Workspace {
    listed: Listed,
    listed_once: RwSignal<bool>,
    problem: RwSignal<Option<ApiError>>,
    opening: RwSignal<Opening>,
    creating: Act<(String, ProjectDetails)>,
    creation_refused: RwSignal<Option<ApiError>>,
    trashed: Trashed,
    listing: Action<Option<Awaited>, ()>,
    asking_the_trash: Action<Option<AwaitedInTheTrash>, ()>,
}

impl Workspace {
    pub fn listing(with_accounts: bool) -> Self {
        let problem = RwSignal::new(None);
        let failed = move |failure| _ = problem.try_set(Some(failure));
        let listed: Listed = WorkingCopies::new(
            |id: ProjectId, on: Project, intent: Intent| async move {
                match intent {
                    Intent::Rename { name } => service::rename(&id, &name, on.version).await,
                    Intent::Trash => {
                        service::trash(&id, on.version).await?;
                        Ok(Project {
                            version: on.version.next(),
                            trashed: true,
                            ..on
                        })
                    }
                    Intent::ChangeDetails { .. } | Intent::Restore => Err(ApiError::Unexpected),
                }
            },
            |id: ProjectId| async move { service::still_listed(&id).await },
            failed,
        );
        let trashed: Trashed = WorkingCopies::new(
            |id: ProjectId, on: TrashedProject, intent: Intent| async move {
                match intent {
                    Intent::Restore => {
                        service::restore(&id, on.version).await?;
                        Ok(TrashedProject {
                            version: on.version.next(),
                            restored: true,
                            ..on
                        })
                    }
                    Intent::Rename { .. } | Intent::ChangeDetails { .. } | Intent::Trash => {
                        Err(ApiError::Unexpected)
                    }
                }
            },
            |id: ProjectId| async move {
                service::in_the_trash()
                    .await?
                    .into_iter()
                    .find(|held| held.id == id)
                    .ok_or(ApiError::NotFound("project"))
            },
            failed,
        );
        let listed_once = RwSignal::new(false);
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
                    Ok(found) => listed.take_in_all(by_id(found, |project| &project.id)),
                    Err(failure) => _ = problem.try_set(Some(failure)),
                }
                _ = listed_once.try_set(true);
            }
        });
        let trash_asking = Asking::new();
        let asking_the_trash = Action::new_local(move |awaited: &Option<AwaitedInTheTrash>| {
            let asked = trash_asking.next();
            let awaited = awaited.clone();

            async move {
                let answer = asking::until_it_holds(
                    service::in_the_trash,
                    |answer: &Result<Vec<TrashedProject>, ApiError>| match (answer, &awaited) {
                        (Ok(found), Some(awaited)) => awaited.holds(found),
                        _ => true,
                    },
                    asking::paused,
                )
                .await;
                if !trash_asking.is_latest(asked) {
                    return;
                }
                match answer {
                    Ok(found) => trashed.take_in_all(by_id(found, |project| &project.id)),
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
                let awaited = Awaited::told(&told);
                let next = listed_once
                    .try_get_untracked()
                    .unwrap_or(false)
                    .then(|| model::taken_in(confirmed(listed), told))
                    .flatten();
                match next {
                    Some(next) => listed.take_in_all(by_id(next, |project| &project.id)),
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
                        take_in_joined(listed, project, Duration::ZERO)
                    }
                    Some(Membership::Ended(project)) => listed.forget(&project),
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
            listed,
            listed_once,
            problem,
            opening,
            creating: Act::new(move |(name, details): (String, ProjectDetails), _: Here| {
                opening.set(Opening::Idle);
                async move {
                    let outcome = service::create(&name, &details).await;
                    if let Ok(created) = &outcome {
                        listed.take_in(created.id.clone(), created.clone());
                        opening.set(Opening::SettingUp);
                        open_once_it_answers(opening, created.clone(), Duration::ZERO);
                    }

                    _ = creation_refused.try_set(outcome.err());
                }
            }),
            creation_refused,
            trashed,
            listing,
            asking_the_trash,
        }
    }

    pub fn projects(self) -> Vec<Project> {
        let mut shown: Vec<Project> = self
            .listed
            .shown()
            .into_iter()
            .filter(|project| !project.trashed)
            .collect();
        shown.sort_by(|one, other| other.id.cmp(&one.id));

        shown
    }

    pub fn problem(self) -> Option<ApiError> {
        self.problem.get()
    }

    pub fn loading(self) -> bool {
        !self.listed_once.get()
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

    /// The `aria-busy` of what renames or trashes a project: set while a change of the author's to it is unanswered.
    pub fn changing(self, id: &ProjectId) -> Option<&'static str> {
        self.listed.unanswered_on(id).then_some("true")
    }

    pub fn rename(self, id: ProjectId, name: String) {
        self.problem.set(None);
        self.listed.intend(&id, Intent::Rename { name });
    }

    pub fn trash(self, id: ProjectId) {
        if untrack(|| self.listed.unanswered_on(&id)) {
            return;
        }
        self.problem.set(None);
        let Some(landed) = self.listed.intended(&id, Intent::Trash) else {
            return;
        };
        let (listed, asking_the_trash) = (self.listed, self.asking_the_trash);

        task::spawn_local(async move {
            if landed.await {
                listed.forget(&id);
                asking_the_trash.dispatch(Some(AwaitedInTheTrash::In(id)));
            }
        });
    }

    pub fn in_the_trash(self) -> Vec<TrashedProject> {
        self.trashed
            .shown()
            .into_iter()
            .filter(|project| !project.restored)
            .collect()
    }

    pub fn trash_asked(self) -> bool {
        self.trashed.is_read()
    }

    pub fn restoring(self, id: &ProjectId) -> Option<&'static str> {
        self.trashed.unanswered_on(id).then_some("true")
    }

    pub fn restore(self, id: ProjectId) {
        if untrack(|| self.trashed.unanswered_on(&id)) {
            return;
        }
        self.problem.set(None);
        let Some(landed) = self.trashed.intended(&id, Intent::Restore) else {
            return;
        };
        let (trashed, listing) = (self.trashed, self.listing);

        task::spawn_local(async move {
            if landed.await {
                trashed.forget(&id);
                listing.dispatch(Some(Awaited::Listed {
                    id,
                    at_least: Version::from(0),
                }));
            }
        });
    }
}

fn by_id<T>(found: Vec<T>, id: impl Fn(&T) -> &ProjectId) -> Vec<(ProjectId, T)> {
    found
        .into_iter()
        .map(|held| (id(&held).clone(), held))
        .collect()
}

fn confirmed(listed: Listed) -> Vec<Project> {
    listed
        .confirmed_all()
        .into_iter()
        .map(|(_, project)| project)
        .collect()
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

fn take_in_joined(listed: Listed, project: ProjectId, waited: Duration) {
    task::spawn_local(async move {
        match service::one(&project).await {
            Ok(found) => listed.take_in(project, found),
            Err(ApiError::NotFound(_)) => {
                if let Some(wait) = model::next_wait(waited) {
                    _ = set_timeout(move || take_in_joined(listed, project, waited + wait), wait);
                }
            }
            Err(_) => {}
        }
    });
}
