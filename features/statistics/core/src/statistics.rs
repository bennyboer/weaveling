use std::sync::Arc;

use access::{KeptAccess, Role};
use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{AccountId, Agent, Version};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::catalog::WordCountCatalog;
use crate::error::StatisticsError;
use crate::link::{ProjectLink, SceneLink};
use crate::word_count::WordCount;

#[derive(Clone)]
pub struct StatisticsCore {
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn WordCountCatalog>,
    access: KeptAccess,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl StatisticsCore {
    pub fn new(
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn WordCountCatalog>,
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

    pub async fn word_counts(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Vec<WordCount>, StatisticsError> {
        if !self.may_follow(project, agent).await? {
            return Err(StatisticsError::ProjectNotThere);
        }

        Ok(self.catalog.in_project(&ProjectLink::from(project)).await?)
    }

    pub async fn may_follow(&self, project: &str, agent: &Agent) -> Result<bool, StatisticsError> {
        Ok(self.authorizer.may(agent, Act::Read, project).await?)
    }

    pub fn watch_access(&self, project: &str) -> AccessWatch {
        self.authorizer.watch_access(project)
    }

    pub async fn counted(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        project: &str,
        words: u64,
        at: OffsetDateTime,
    ) -> Result<(), StatisticsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .keep(
                    transaction,
                    &SceneLink::from(scene),
                    &ProjectLink::from(project),
                    words,
                    at,
                )
                .await?;

            Ok(())
        })
    }

    pub async fn scene_deleted(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        at: OffsetDateTime,
    ) -> Result<(), StatisticsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .forget_scene(transaction, &SceneLink::from(scene), at)
                .await?;

            Ok(())
        })
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, StatisticsError> {
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
    ) -> Result<(), StatisticsError> {
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

            Ok::<_, StatisticsError>(())
        })
    }

    /// Remembers a project's members disbanded, its project deleted: nobody is a member of it any more.
    pub async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), StatisticsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_disbanded(transaction, project, version, self.clock.now())
                .await?;

            Ok::<_, StatisticsError>(())
        })
    }

    /// Remembers whether a project is in the trash, as the projects told it at a version.
    pub async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), StatisticsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.access
                .remember_trash(transaction, project, version, in_the_trash)
                .await?;

            Ok::<_, StatisticsError>(())
        })
    }
}
