use crate::http::ApiError;
use crate::ideas::model::{Idea, IdeaId};
use crate::ideas::service;
use crate::working_copy;
use crate::working_copy::{Versioned, WorkingCopies};

/// The ideas the author works on, each a working copy of its own: an idea is its own aggregate, so a change to one never waits for another.
pub type Ideas = WorkingCopies<IdeaId, Idea, Intent>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    Retitle { title: String },
}

/// Built where the view that changes the ideas lives, as every signal belongs to the owner current when it is made.
pub fn copies(failed: impl Fn(ApiError) + 'static) -> Ideas {
    WorkingCopies::new(
        |_, on: Idea, intent: Intent| async move { intent.made_on(&on).await },
        |about: IdeaId| async move { service::get(&about).await },
        failed,
    )
}

impl Versioned for Idea {
    fn version(&self) -> u64 {
        self.version
    }
}

impl working_copy::Intent<Idea> for Intent {
    fn lay_over(&self, idea: &mut Idea) {
        match self {
            Self::Retitle { title } => idea.title.clone_from(title),
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        match (self, earlier) {
            (Self::Retitle { .. }, Self::Retitle { .. }) => Some(self.clone()),
        }
    }
}

impl Intent {
    async fn made_on(&self, on: &Idea) -> Result<Idea, ApiError> {
        match self {
            Self::Retitle { title } => service::retitle(&on.id, title, on.version).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::model::ProjectId;
    use crate::working_copy::Intent as _;

    fn an_idea(title: &str) -> Idea {
        Idea {
            id: IdeaId::from("idea_1".to_owned()),
            version: 4,
            project: ProjectId::from("project_1".to_owned()),
            title: title.to_owned(),
        }
    }

    fn a_retitle(title: &str) -> Intent {
        Intent::Retitle {
            title: title.to_owned(),
        }
    }

    #[test]
    fn a_retitle_laid_over_shows_the_idea_under_its_new_title() {
        // given: an idea as confirmed
        let mut shown = an_idea("The loom");

        // when: a retitle is laid over it
        a_retitle("The weft").lay_over(&mut shown);

        // then: it shows the new title at the version confirmed
        assert_eq!(shown, an_idea("The weft"));
    }

    #[test]
    fn a_retitle_after_another_is_sent_as_the_last() {
        // given: a retitle waiting
        let earlier = a_retitle("The weft");

        // when: another follows it
        let merged = a_retitle("The heddle").merged_after(&earlier);

        // then: only the last title goes out
        assert_eq!(merged, Some(a_retitle("The heddle")));
    }
}
