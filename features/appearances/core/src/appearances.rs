use std::sync::Arc;

use authorizing::{Act, Authorizer};
use eventsourcing::Agent;
use time::OffsetDateTime;
use transacting::Transactions;

use crate::appearance::{Place, Subject};
use crate::catalog::AppearanceCatalog;
use crate::error::AppearancesError;
use crate::link::{IdeaLink, ProjectLink};

#[derive(Clone)]
pub struct AppearancesCore {
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn AppearanceCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl AppearancesCore {
    pub fn new(
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn AppearanceCatalog>,
        authorizer: Arc<dyn Authorizer>,
    ) -> Self {
        Self {
            transactions,
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

        let mut transaction = self.transactions.begin().await?;
        if let Some(project) = project {
            self.catalog
                .remember_project(&mut transaction, &idea, &ProjectLink::from(project))
                .await?;
        }
        self.catalog
            .remember(&mut transaction, &idea, place, version)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn disappeared(
        &self,
        idea: &str,
        place: &Place,
        version: u64,
    ) -> Result<(), AppearancesError> {
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .forget(&mut transaction, &an_idea(idea), place, version)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn place_gone(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<(), AppearancesError> {
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .forget_place(&mut transaction, place, at)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn idea_discarded(
        &self,
        idea: &str,
        at: OffsetDateTime,
    ) -> Result<(), AppearancesError> {
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .forget_subject(&mut transaction, &an_idea(idea), at)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AppearancesError> {
        let mut transaction = self.transactions.begin().await?;
        let forgotten = self
            .catalog
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await?;

        Ok(forgotten)
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
