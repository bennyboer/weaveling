use leptos::prelude::*;
use leptos::task;

use crate::asking::Asking;
use crate::http::ApiError;
use crate::live::Subscription;
use crate::projects::intent::Intent;
use crate::projects::model::{DescribedProject, ProjectId, Version};
use crate::projects::service;
use crate::working_copy::{Landed, WorkingCopies};
use live_contract::FrameDTO;
use quotation::QuotationMarks;

const SUBJECT: &str = "project";

#[derive(Clone, Copy)]
pub struct OpenProject {
    project: RwSignal<Option<ProjectId>>,
    held: RwSignal<Option<ProjectId>>,
    working: WorkingCopies<ProjectId, DescribedProject, Intent>,
    refused: RwSignal<Option<ApiError>>,
    change_refused: RwSignal<Option<ApiError>>,
    details_built_on: StoredValue<Option<Version>>,
    following: StoredValue<Option<Subscription>, LocalStorage>,
    asking: Asking,
}

impl OpenProject {
    pub fn none() -> Self {
        let change_refused = RwSignal::new(None);
        let details_built_on = StoredValue::new(None);
        let working = WorkingCopies::new(
            move |id: ProjectId, on: DescribedProject, intent: Intent| async move {
                if intent.changed_meanwhile(&on) {
                    return Err(ApiError::Outdated(SUBJECT));
                }
                match intent {
                    Intent::Rename { name } => {
                        let renamed = service::rename(&id, &name, on.version).await?;
                        Ok(DescribedProject {
                            version: renamed.version,
                            name: renamed.name,
                            ..on
                        })
                    }
                    Intent::ChangeDetails { details, .. } => {
                        _ = details_built_on.try_set_value(Some(on.version));
                        service::change_details(&id, &details, on.version).await
                    }
                    Intent::Trash => {
                        service::trash(&id, on.version).await?;
                        Ok(DescribedProject {
                            version: on.version.next(),
                            ..on
                        })
                    }
                    Intent::Restore => Err(ApiError::Unexpected),
                }
            },
            |id: ProjectId| async move { service::details(&id).await },
            move |failure| _ = change_refused.try_set(Some(failure)),
        );

        Self {
            project: RwSignal::new(None),
            held: RwSignal::new(None),
            working,
            refused: RwSignal::new(None),
            change_refused,
            details_built_on,
            following: StoredValue::new_local(None),
            asking: Asking::new(),
        }
    }

    pub fn open(self, project: &ProjectId) {
        if self
            .project
            .with_untracked(|held| held.as_ref() == Some(project))
        {
            return;
        }
        if let Some(before) = self.held.get_untracked() {
            self.working.forget(&before);
        }
        self.project.set(Some(project.clone()));
        self.held.set(Some(project.clone()));
        self.refused.set(None);
        self.change_refused.set(None);
        self.following.set_value(Some(Subscription::start(
            "projects",
            &[("project", project.to_string().as_str())],
            move |told| self.heard(told),
            move || self.ask(),
        )));
    }

    fn ask(self) {
        let Some(project) = self.project.get_untracked() else {
            return;
        };
        let asked = self.asking.next();

        task::spawn_local(async move {
            let found = service::details(&project).await;
            let still_open = self
                .project
                .with_untracked(|held| held.as_ref() == Some(&project));

            if !still_open || !self.asking.is_latest(asked) {
                return;
            }
            match found {
                Ok(described) => self.working.take_in(project, described),
                Err(failure) => self.let_go(failure),
            }
        });
    }

    fn heard(self, told: FrameDTO) {
        let Some(project) = self.project.get_untracked() else {
            return;
        };
        let Some(held) = self.working.confirmed_one(&project) else {
            return self.ask();
        };
        if held.trashed_by(&told) {
            return self.let_go(service::moved_to_the_trash());
        }
        match held.clone().taken_in(told) {
            Some(next) if next == held => {}
            Some(next) => self.working.take_in(project, next),
            None => self.ask(),
        }
    }

    fn let_go(self, failure: ApiError) {
        self.project.set(None);
        self.following.set_value(None);
        self.refused.set(Some(failure));
    }

    pub fn described(self) -> Option<DescribedProject> {
        self.held
            .get()
            .and_then(|project| self.working.shown_one(&project))
    }

    pub fn name(self) -> Option<String> {
        self.described().map(|held| held.name)
    }

    pub fn language(self) -> Option<String> {
        self.described().and_then(|held| held.details.language)
    }

    pub fn quotation_marks(self) -> QuotationMarks {
        self.described().map_or_else(
            || QuotationMarks::of_language(None),
            |held| held.details.quotation_marks(),
        )
    }

    pub fn intended(self, project: &ProjectId, intent: Intent) -> Option<Landed> {
        self.change_refused.set(None);
        let landed = self
            .held
            .with_untracked(|held| held.as_ref() == Some(project))
            .then(|| self.working.intended(project, intent))
            .flatten();
        if landed.is_none() {
            self.change_refused.set(Some(ApiError::Unexpected));
        }

        landed
    }

    pub fn change_refused(self) -> Option<ApiError> {
        self.change_refused.get_untracked()
    }

    pub fn details_built_on(self) -> Option<Version> {
        self.details_built_on.get_value()
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }
}
