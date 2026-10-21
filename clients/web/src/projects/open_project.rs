use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::http::ApiError;
use crate::live::{Following, Told};
use crate::projects::model::{DescribedProject, ProjectId};
use crate::projects::service;
use quotation::QuotationMarks;

#[derive(Clone, Copy)]
pub struct OpenProject {
    project: RwSignal<Option<ProjectId>>,
    described: RwSignal<Option<DescribedProject>>,
    refused: RwSignal<Option<ApiError>>,
    following: StoredValue<Option<Following>, LocalStorage>,
    latest: StoredValue<u64>,
}

impl OpenProject {
    pub fn none() -> Self {
        Self {
            project: RwSignal::new(None),
            described: RwSignal::new(None),
            refused: RwSignal::new(None),
            following: StoredValue::new_local(None),
            latest: StoredValue::new(0),
        }
    }

    pub fn open(self, project: &ProjectId) {
        if self
            .project
            .with_untracked(|held| held.as_ref() == Some(project))
        {
            return;
        }
        self.project.set(Some(project.clone()));
        self.described.set(None);
        self.refused.set(None);
        self.following.set_value(Some(Following::start(
            &format!("/api/projects/live?project={project}"),
            move |told| self.heard(told),
            move || self.ask(),
        )));
    }

    fn ask(self) {
        let Some(project) = self.project.get_untracked() else {
            return;
        };
        let asked = self.latest.get_value() + 1;
        self.latest.set_value(asked);

        spawn_local(async move {
            let found = service::details(&project).await;
            let still_open = self
                .project
                .with_untracked(|held| held.as_ref() == Some(&project));

            if !still_open || self.latest.get_value() != asked {
                return;
            }
            match found {
                Ok(described) => self.described.set(Some(described)),
                Err(failure) => {
                    self.project.set(None);
                    self.following.set_value(None);
                    self.refused.set(Some(failure));
                }
            }
        });
    }

    fn heard(self, told: Told) {
        let Some(held) = self.described.get_untracked() else {
            return self.ask();
        };
        match held.clone().taken_in(told) {
            Some(next) if next == held => {}
            Some(next) => self.described.set(Some(next)),
            None => self.ask(),
        }
    }

    pub fn described(self) -> Option<DescribedProject> {
        self.described.get()
    }

    pub fn name(self) -> Option<String> {
        self.described
            .with(|described| described.as_ref().map(|held| held.name.clone()))
    }

    pub fn language(self) -> Option<String> {
        self.described.with(|described| {
            described
                .as_ref()
                .and_then(|held| held.details.language.clone())
        })
    }

    pub fn quotation_marks(self) -> QuotationMarks {
        self.described.with(|described| {
            described.as_ref().map_or_else(
                || QuotationMarks::of_language(None),
                |held| held.details.quotation_marks(),
            )
        })
    }

    pub fn keep(self, described: DescribedProject) {
        let newer = self.described.with_untracked(|held| {
            held.as_ref()
                .is_none_or(|held| held.version < described.version)
        });

        if newer {
            self.described.set(Some(described));
        }
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }
}
