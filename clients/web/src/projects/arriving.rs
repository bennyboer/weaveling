use std::time::Duration;

use leptos::prelude::*;
use leptos::task;

use crate::http::{ApiError, Refusal};
use crate::projects::model;
use crate::projects::model::{Project, ProjectId};
use crate::projects::service;
use crate::projects::workspace::Listed;

const NOT_YET_OPEN: &str = "project-not-yet-open";

#[derive(Clone, Debug)]
pub(super) enum Opening {
    Idle,
    SettingUp,
    Opened(Project),
    Unopened(ApiError),
}

pub(super) fn open_once_it_answers(opening: RwSignal<Opening>, project: Project, waited: Duration) {
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

pub(super) fn take_in_joined(listed: Listed, project: ProjectId, waited: Duration) {
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
