use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::http::ApiError;
use crate::projects::model::{ProjectDetails, ProjectId};
use crate::projects::service;

#[derive(Clone, Copy)]
pub struct OpenProject {
    project: RwSignal<Option<ProjectId>>,
    details: RwSignal<Option<ProjectDetails>>,
    refused: RwSignal<Option<ApiError>>,
}

impl OpenProject {
    pub fn none() -> Self {
        Self {
            project: RwSignal::new(None),
            details: RwSignal::new(None),
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
        self.details.set(None);
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
                Ok(described) => self.details.set(Some(described)),
                Err(failure) => {
                    self.project.set(None);
                    self.refused.set(Some(failure));
                }
            }
        });
    }

    pub fn language(self) -> Option<String> {
        self.details
            .with(|details| details.as_ref().and_then(|held| held.language.clone()))
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }
}
