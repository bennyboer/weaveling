use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use eventsourcing::Agent;
use time::OffsetDateTime;

use crate::catalog::WordCountCatalog;
use crate::error::StatisticsError;
use crate::link::{ProjectLink, SceneLink};
use crate::word_count::WordCount;

#[derive(Clone)]
pub struct StatisticsCore {
    catalog: Arc<dyn WordCountCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl StatisticsCore {
    pub fn new(catalog: Arc<dyn WordCountCatalog>, authorizer: Arc<dyn Authorizer>) -> Self {
        Self {
            catalog,
            authorizer,
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
        scene: &str,
        project: &str,
        words: u64,
        at: OffsetDateTime,
    ) -> Result<(), StatisticsError> {
        self.catalog
            .keep(
                &SceneLink::from(scene),
                &ProjectLink::from(project),
                words,
                at,
            )
            .await?;

        Ok(())
    }

    pub async fn scene_deleted(
        &self,
        scene: &str,
        at: OffsetDateTime,
    ) -> Result<(), StatisticsError> {
        Ok(self
            .catalog
            .forget_scene(&SceneLink::from(scene), at)
            .await?)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, StatisticsError> {
        Ok(self
            .catalog
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
