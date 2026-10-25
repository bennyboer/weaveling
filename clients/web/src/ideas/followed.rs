use leptos::prelude::*;

use crate::http::ApiError;
use crate::ideas::model;
use crate::ideas::model::Idea;
use crate::ideas::service;
use crate::live::Following;
use crate::projects::model::ProjectId;
use live_contract::FrameDTO;

pub fn asking(
    project: &ProjectId,
    held: RwSignal<Option<Vec<Idea>>>,
    failed: impl Fn(ApiError) + Clone + 'static,
) -> Action<(), ()> {
    let project = project.clone();
    let latest = StoredValue::new(0_u64);

    Action::new_local(move |()| {
        let project = project.clone();
        let failed = failed.clone();
        let asked = latest.get_value() + 1;
        latest.set_value(asked);

        async move {
            let answer = service::list(&project).await;
            if latest.try_get_value() != Some(asked) {
                return;
            }
            match answer {
                Ok(found) => _ = held.try_set(Some(found)),
                Err(failure) => failed(failure),
            }
        }
    })
}

pub fn following(
    project: &ProjectId,
    held: RwSignal<Option<Vec<Idea>>>,
    asking: Action<(), ()>,
) -> Following {
    Following::start(
        &format!("/api/ideas/live?project={project}"),
        move |told: FrameDTO| {
            let Some(known) = held.try_get_untracked() else {
                return;
            };
            match known.and_then(|known| model::taken_in(known, told)) {
                Some(next) => _ = held.try_set(Some(next)),
                None => _ = asking.dispatch(()),
            }
        },
        move || _ = asking.dispatch(()),
    )
}
