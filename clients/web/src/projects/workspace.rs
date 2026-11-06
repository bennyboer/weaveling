use std::future::Future;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task;

use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::live::Subscription;
use crate::projects::arriving;
use crate::projects::arriving::Opening;
use crate::projects::intent::Intent;
use crate::projects::model;
use crate::projects::model::{
    Membership, Project, ProjectDetails, ProjectId, Told, TrashedProject,
};
use crate::projects::service;
use crate::working_copy::{Awaited, WorkingCopies};
use live_contract::FrameDTO;

pub(super) type Listed = WorkingCopies<ProjectId, Project, Intent>;
type Trashed = WorkingCopies<ProjectId, TrashedProject, Intent>;

#[derive(Clone, Copy)]
pub struct Workspace {
    listed: Listed,
    problem: RwSignal<Option<ApiError>>,
    opening: RwSignal<Opening>,
    creating: Act<(String, ProjectDetails)>,
    creation_refused: RwSignal<Option<ApiError>>,
    trashed: Trashed,
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
                        Ok(on)
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
                        Ok(on)
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
        let opening = RwSignal::new(Opening::Idle);
        let creation_refused = RwSignal::new(None);

        let workspace = Self {
            listed,
            problem,
            opening,
            creating: Act::new(move |(name, details): (String, ProjectDetails), _: Here| {
                opening.set(Opening::Idle);
                async move {
                    let outcome = service::create(&name, &details).await;
                    if let Ok(created) = &outcome {
                        listed.take_in(created.id.clone(), created.clone());
                        opening.set(Opening::SettingUp);
                        arriving::open_once_it_answers(opening, created.clone(), Duration::ZERO);
                    }

                    _ = creation_refused.try_set(outcome.err());
                }
            }),
            creation_refused,
            trashed,
        };
        workspace.read_the_trash(None);
        StoredValue::new_local(Subscription::start(
            "projects",
            &[("mine", "true")],
            move |told: FrameDTO| workspace.heard(told),
            move || workspace.read_the_list(None),
        ));
        if with_accounts {
            let reconnected = StoredValue::new(false);
            StoredValue::new_local(Subscription::start(
                "members",
                &[("mine", "true")],
                move |told: FrameDTO| match model::membership_in(told) {
                    Some(Membership::Begun(project)) => {
                        arriving::take_in_joined(listed, project, Duration::ZERO)
                    }
                    Some(Membership::Ended(project)) => listed.forget(&project),
                    None => {}
                },
                move || {
                    if reconnected.get_value() {
                        workspace.read_the_list(None);
                    }
                    reconnected.set_value(true);
                },
            ));
        }

        workspace
    }

    fn heard(self, told: FrameDTO) {
        if let Some(awaited) = model::awaited_in_the_trash(&told) {
            self.read_the_trash(Some(awaited));
        }
        let awaited = model::awaited_in_the_list(&told);
        if !untrack(|| self.listed.is_read()) {
            return self.read_the_list(awaited);
        }
        let about = told.of.clone().map(ProjectId::from);
        let held = about
            .as_ref()
            .and_then(|about| self.listed.confirmed_one(about));
        match about.zip(model::told_about(held.as_ref(), told)) {
            Some((about, Told::Changed(project))) => self.listed.take_in(about, project),
            Some((about, Told::Gone)) => self.listed.forget(&about),
            Some((_, Told::Unchanged)) => {}
            None => self.read_the_list(awaited),
        }
    }

    fn read_the_list(self, awaited: Option<Awaited<ProjectId>>) {
        let reading = self.listed.read_all(
            || async { Ok(by_id(service::list().await?, |project| &project.id)) },
            awaited,
        );
        self.told_if_failed(reading);
    }

    fn read_the_trash(self, awaited: Option<Awaited<ProjectId>>) {
        let reading = self.trashed.read_all(
            || async { Ok(by_id(service::in_the_trash().await?, |project| &project.id)) },
            awaited,
        );
        self.told_if_failed(reading);
    }

    fn told_if_failed(self, reading: impl Future<Output = Result<(), ApiError>> + 'static) {
        let problem = self.problem;
        task::spawn_local(async move {
            if let Err(failure) = reading.await {
                _ = problem.try_set(Some(failure));
            }
        });
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
        !self.listed.is_read() && self.problem.with(Option::is_none)
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
        self.problem.set(None);
        let ending = self.listed.ending(&id, Intent::Trash);

        task::spawn_local(async move {
            if ending.await {
                self.read_the_trash(Some(Awaited::Held { id, at_least: 0 }));
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
        self.problem.set(None);
        let ending = self.trashed.ending(&id, Intent::Restore);

        task::spawn_local(async move {
            if ending.await {
                self.read_the_list(Some(Awaited::Held { id, at_least: 0 }));
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
