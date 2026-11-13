use leptos::prelude::*;
use leptos::task;

use crate::http::ApiError;
use crate::ideas::model;
use crate::ideas::model::{Idea, IdeaId, Told};
use crate::ideas::service;
use crate::ideas::working;
use crate::ideas::working::Ideas;
use crate::live::Subscription;
use crate::projects::model::ProjectId;
use crate::working_copy::Awaited;
use live_contract::FrameDTO;

/// The ideas of a project, as the author works on them.
///
/// They are read in full when the live socket opens.
/// They are then kept up to date by version as other authors change them.
/// Each idea is changed through its own working copy.
#[derive(Clone, Copy)]
pub struct FollowedIdeas {
    project: StoredValue<ProjectId>,
    copies: Ideas,
}

impl FollowedIdeas {
    pub fn follow(project: &ProjectId, failed: impl Fn(ApiError) + 'static) -> Self {
        let followed = Self {
            project: StoredValue::new(project.clone()),
            copies: working::copies(failed),
        };
        StoredValue::new_local(Subscription::start(
            "ideas",
            &[("project", project.to_string().as_str())],
            move |told: FrameDTO| followed.heard(told),
            move || followed.read(None),
        ));

        followed
    }

    pub fn copies(self) -> Ideas {
        self.copies
    }

    pub fn is_read(self) -> bool {
        self.copies.is_read()
    }

    pub fn read_again(self) {
        self.read(None);
    }

    /// Reads the ideas again, and repeats the request until the list contains `idea`.
    /// This is needed because a new idea can take a moment to show up in the list.
    pub fn read_until_listed(self, idea: &IdeaId) {
        self.read(Some(Awaited::Held {
            id: idea.clone(),
            at_least: 1,
        }));
    }

    fn heard(self, told: FrameDTO) {
        let awaited = model::awaited(&told);
        if !untrack(|| self.copies.is_read()) {
            return self.read(awaited);
        }
        let about = told.of.clone().map(IdeaId::from);
        let held = about
            .as_ref()
            .and_then(|about| self.copies.confirmed_one(about));
        match about.zip(model::told_about(held.as_ref(), told)) {
            Some((about, Told::Changed(idea))) => self.copies.take_in(about, idea),
            Some((about, Told::Discarded)) => self.copies.forget(&about),
            Some((_, Told::Unchanged)) => {}
            None => self.read(awaited),
        }
    }

    fn read(self, awaited: Option<Awaited<IdeaId>>) {
        let Some(project) = self.project.try_get_value() else {
            return;
        };
        let copies = self.copies;
        let reading = copies.read_all(move || listed(project.clone()), awaited);

        task::spawn_local(async move {
            if let Err(failure) = reading.await {
                copies.fail(failure);
            }
        });
    }
}

async fn listed(project: ProjectId) -> Result<Vec<(IdeaId, Idea)>, ApiError> {
    Ok(service::list(&project)
        .await?
        .into_iter()
        .map(|idea| (idea.id.clone(), idea))
        .collect())
}
