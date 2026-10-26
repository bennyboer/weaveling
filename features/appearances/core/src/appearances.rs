use std::sync::Arc;

use authorizing::{Act, Authorizer};
use eventsourcing::Agent;
use time::OffsetDateTime;

use crate::appearance::{Place, Subject};
use crate::catalog::AppearanceCatalog;
use crate::error::AppearancesError;
use crate::link::{IdeaLink, ProjectLink};

#[derive(Clone)]
pub struct AppearancesCore {
    catalog: Arc<dyn AppearanceCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl AppearancesCore {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>, authorizer: Arc<dyn Authorizer>) -> Self {
        Self {
            catalog,
            authorizer,
        }
    }

    pub async fn places_of(
        &self,
        idea: &str,
        agent: &Agent,
    ) -> Result<Vec<Place>, AppearancesError> {
        if !self.may_follow(idea, agent).await? {
            return Err(AppearancesError::IdeaNotThere);
        }

        Ok(self.catalog.places_of(&an_idea(idea)).await?)
    }

    pub async fn may_follow(&self, idea: &str, agent: &Agent) -> Result<bool, AppearancesError> {
        Ok(self.may_read(idea, agent).await?.unwrap_or(true))
    }

    pub async fn may_hear(&self, idea: &str, agent: &Agent) -> Result<bool, AppearancesError> {
        Ok(self.may_read(idea, agent).await?.unwrap_or(false))
    }

    pub async fn appeared(
        &self,
        idea: &str,
        place: &Place,
        version: u64,
        project: Option<&str>,
    ) -> Result<(), AppearancesError> {
        let idea = an_idea(idea);
        if let Some(project) = project {
            self.catalog
                .remember_project(&idea, &ProjectLink::from(project))
                .await?;
        }

        Ok(self.catalog.remember(&idea, place, version).await?)
    }

    pub async fn disappeared(
        &self,
        idea: &str,
        place: &Place,
        version: u64,
    ) -> Result<(), AppearancesError> {
        Ok(self.catalog.forget(&an_idea(idea), place, version).await?)
    }

    pub async fn place_gone(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<(), AppearancesError> {
        self.catalog.forget_place(place, at).await?;

        Ok(())
    }

    pub async fn idea_discarded(
        &self,
        idea: &str,
        at: OffsetDateTime,
    ) -> Result<(), AppearancesError> {
        Ok(self.catalog.forget_subject(&an_idea(idea), at).await?)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AppearancesError> {
        Ok(self
            .catalog
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }

    async fn may_read(&self, idea: &str, agent: &Agent) -> Result<Option<bool>, AppearancesError> {
        let Some(project) = self.catalog.project_of(&an_idea(idea)).await? else {
            return Ok(None);
        };

        Ok(Some(
            self.authorizer
                .may(agent, Act::Read, project.as_str())
                .await?,
        ))
    }
}

fn an_idea(id: &str) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}
