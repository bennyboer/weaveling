use std::sync::Arc;

use access::{KeptAccess, Role};
use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::{AccountId, Agent, Version};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::appearance::{Place, Subject};
use crate::catalog::AppearanceCatalog;
use crate::error::AppearancesError;
use crate::link::{IdeaLink, ProjectLink};

#[derive(Clone)]
pub struct AppearancesCore {
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn AppearanceCatalog>,
    access: KeptAccess,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl AppearancesCore {
    pub fn new(
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn AppearanceCatalog>,
        access: KeptAccess,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            transactions,
            catalog,
            access,
            authorizer,
            clock,
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

    /// An idea's project is taken from the ideas alone, never from where it is linked, so a link from another project moves nothing.
    pub async fn captured(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
        project: &str,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_project(transaction, &an_idea(idea), &ProjectLink::from(project))
                .await?;

            Ok(())
        })
    }

    /// The project given is the place's, so an idea linked from another project is kept but shown nowhere.
    pub async fn appeared(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
        place: &Place,
        version: u64,
        project: Option<&str>,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            if let Some(project) = project {
                self.catalog
                    .remember_place_project(transaction, place, &ProjectLink::from(project))
                    .await?;
            }
            self.catalog
                .remember(transaction, &an_idea(idea), place, version)
                .await?;

            Ok(())
        })
    }

    pub async fn disappeared(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
        place: &Place,
        version: u64,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget(transaction, &an_idea(idea), place, version)
                .await?;

            Ok(())
        })
    }

    pub async fn place_gone(
        &self,
        within: Option<&mut Transaction>,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog.forget_place(transaction, place, at).await?;

            Ok(())
        })
    }

    /// Every place of a deleted project goes, sections included, whose outline names none of them as it goes.
    pub async fn project_deleted(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        at: OffsetDateTime,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget_places_of_project(transaction, &ProjectLink::from(project), at)
                .await?;

            Ok(())
        })
    }

    pub async fn idea_discarded(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
        at: OffsetDateTime,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget_subject(transaction, &an_idea(idea), at)
                .await?;

            Ok(())
        })
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AppearancesError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let access = self
                .access
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(forgotten + access)
        })
    }

    /// Remembers an account's role in a project, or that it is a member no more, as the project's members told it at a version, so who may see what follows.
    pub async fn remember_role(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_role(
                    transaction,
                    project,
                    account,
                    version,
                    role,
                    self.clock.now(),
                )
                .await?;

            Ok::<_, AppearancesError>(())
        })
    }

    /// Remembers a project's members disbanded, its project deleted: nobody is a member of it any more.
    pub async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_disbanded(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, AppearancesError>(())
        })
    }

    /// Remembers whether a project is in the trash, as the projects told it at a version.
    pub async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AppearancesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_trash(transaction, project, version, in_the_trash)
                .await?;

            Ok::<_, AppearancesError>(())
        })
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
