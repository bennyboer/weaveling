use leptos::prelude::*;

use crate::asking;
use crate::asking::Asking;
use crate::http::ApiError;
use crate::ideas::model;
use crate::ideas::model::{Awaited, Idea, IdeaId, Told};
use crate::ideas::service;
use crate::ideas::working;
use crate::ideas::working::Ideas;
use crate::live::Subscription;
use crate::projects::model::ProjectId;
use live_contract::FrameDTO;

/// The ideas of a project as the author works on them: read whole when the live socket opens, kept up with by version as other authors change them, and each changed through a working copy of its own.
#[derive(Clone, Copy)]
pub struct FollowedIdeas {
    copies: Ideas,
    asking: Action<Option<Awaited>, ()>,
}

impl FollowedIdeas {
    pub fn follow(project: &ProjectId, failed: impl Fn(ApiError) + Clone + 'static) -> Self {
        let copies = working::copies(failed.clone());
        let asking = asked(
            project,
            move |found| {
                copies.take_in_all(
                    found
                        .into_iter()
                        .map(|idea| (idea.id.clone(), idea))
                        .collect(),
                )
            },
            failed,
        );
        StoredValue::new_local(Subscription::start(
            "ideas",
            &[("project", project.to_string().as_str())],
            move |told: FrameDTO| {
                let awaited = Awaited::told(&told);
                if !untrack(|| copies.is_read()) {
                    _ = asking.dispatch(awaited);
                    return;
                }
                let about = told.of.clone().map(IdeaId::from);
                let held = about.as_ref().and_then(|about| copies.confirmed_one(about));
                match about.zip(model::told_about(held.as_ref(), told)) {
                    Some((about, Told::Changed(idea))) => copies.take_in(about, idea),
                    Some((about, Told::Discarded)) => copies.forget(&about),
                    Some((_, Told::Unchanged)) => {}
                    None => _ = asking.dispatch(awaited),
                }
            },
            move || _ = asking.dispatch(None),
        ));

        Self { copies, asking }
    }

    pub fn copies(self) -> Ideas {
        self.copies
    }

    pub fn is_read(self) -> bool {
        self.copies.is_read()
    }

    pub fn read_again(self) {
        self.asking.dispatch(None);
    }
}

pub fn asking(
    project: &ProjectId,
    held: RwSignal<Option<Vec<Idea>>>,
    failed: impl Fn(ApiError) + Clone + 'static,
) -> Action<Option<Awaited>, ()> {
    asked(project, move |found| _ = held.try_set(Some(found)), failed)
}

fn asked(
    project: &ProjectId,
    found: impl Fn(Vec<Idea>) + Clone + 'static,
    failed: impl Fn(ApiError) + Clone + 'static,
) -> Action<Option<Awaited>, ()> {
    let project = project.clone();
    let asking = Asking::new();

    Action::new_local(move |awaited: &Option<Awaited>| {
        let project = project.clone();
        let found = found.clone();
        let failed = failed.clone();
        let awaited = awaited.clone();
        let asked = asking.next();

        async move {
            let answer = asking::until_it_holds(
                || service::list(&project),
                |answer: &Result<Vec<Idea>, ApiError>| match (answer, &awaited) {
                    (Ok(listed), Some(awaited)) => awaited.holds(listed),
                    _ => true,
                },
                asking::paused,
            )
            .await;
            if !asking.is_latest(asked) {
                return;
            }
            match answer {
                Ok(listed) => found(listed),
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
