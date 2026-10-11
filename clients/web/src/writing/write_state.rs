use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::http::ApiError;
use crate::outline::model::Outline;
use crate::outline::service;
use crate::projects::model::ProjectId;

#[derive(Clone, Copy)]
pub struct WriteState {
    outline: RwSignal<Option<Outline>>,
    problem: RwSignal<Option<ApiError>>,
}

impl WriteState {
    pub fn open(project: &ProjectId) -> Self {
        let state = Self {
            outline: RwSignal::new(None),
            problem: RwSignal::new(None),
        };
        let project = project.clone();

        spawn_local(async move {
            match service::open(&project).await {
                Ok(opened) => state.outline.set(Some(opened)),
                Err(failure) => state.problem.set(Some(failure)),
            }
        });

        state
    }

    pub fn outline(self) -> Option<Outline> {
        self.outline.get()
    }

    pub fn problem(self) -> Option<ApiError> {
        self.problem.get()
    }
}
