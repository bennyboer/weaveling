use leptos::prelude::*;

use crate::asking;
use crate::http::ApiError;
use crate::ideas::model;
use crate::ideas::model::{Awaited, Idea};
use crate::ideas::service;
use crate::live::Subscription;
use crate::projects::model::ProjectId;
use live_contract::FrameDTO;

pub fn asking(
    project: &ProjectId,
    held: RwSignal<Option<Vec<Idea>>>,
    failed: impl Fn(ApiError) + Clone + 'static,
) -> Action<Option<Awaited>, ()> {
    let project = project.clone();
    let latest = StoredValue::new(0_u64);

    Action::new_local(move |awaited: &Option<Awaited>| {
        let project = project.clone();
        let failed = failed.clone();
        let awaited = awaited.clone();
        let asked = latest.get_value() + 1;
        latest.set_value(asked);

        async move {
            let answer = asking::until_it_holds(
                || service::list(&project),
                |answer: &Result<Vec<Idea>, ApiError>| match (answer, &awaited) {
                    (Ok(found), Some(awaited)) => awaited.holds(found),
                    _ => true,
                },
                asking::paused,
            )
            .await;
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
    asking: Action<Option<Awaited>, ()>,
) -> Subscription {
    Subscription::start(
        "ideas",
        &[("project", project.to_string().as_str())],
        move |told: FrameDTO| {
            let Some(known) = held.try_get_untracked() else {
                return;
            };
            let awaited = Awaited::told(&told);
            match known.and_then(|known| model::taken_in(known, told)) {
                Some(next) => _ = held.try_set(Some(next)),
                None => _ = asking.dispatch(awaited),
            }
        },
        move || _ = asking.dispatch(None),
    )
}
