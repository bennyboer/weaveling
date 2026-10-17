use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::http::ApiError;
use crate::projects::model::{DescribedProject, ProjectId};
use crate::projects::quotation::QuotationMarks;
use crate::projects::service;

#[derive(Clone, Copy)]
pub struct OpenProject {
    project: RwSignal<Option<ProjectId>>,
    described: RwSignal<Option<DescribedProject>>,
    refused: RwSignal<Option<ApiError>>,
}

impl OpenProject {
    pub fn none() -> Self {
        Self {
            project: RwSignal::new(None),
            described: RwSignal::new(None),
            refused: RwSignal::new(None),
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

        let project = project.clone();
        spawn_local(async move {
            let found = service::details(&project).await;
            let still_open = self
                .project
                .with_untracked(|held| held.as_ref() == Some(&project));

            if !still_open {
                return;
            }
            match found {
                Ok(described) => self.described.set(Some(described)),
                Err(failure) => {
                    self.project.set(None);
                    self.refused.set(Some(failure));
                }
            }
        });
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
            let details = described.as_ref().map(|held| &held.details);

            details
                .and_then(|details| details.quotation)
                .unwrap_or_else(|| {
                    QuotationMarks::of_language(
                        details.and_then(|details| details.language.as_deref()),
                    )
                })
        })
    }

    pub fn keep(self, described: DescribedProject) {
        self.described.set(Some(described));
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }
}
