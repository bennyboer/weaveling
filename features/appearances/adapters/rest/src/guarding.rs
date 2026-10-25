use std::error::Error;
use std::sync::Arc;

use appearances_core::{AppearanceCatalog, IdeaLink, Subject};
use async_trait::async_trait;
use authorizing::{Act, Authorizer};
use eventsourcing::Agent;
use live::Sieve;
use live_contract::FrameDTO;

#[derive(Clone)]
pub struct Readers {
    pub catalog: Arc<dyn AppearanceCatalog>,
    pub authorizer: Arc<dyn Authorizer>,
}

impl Readers {
    pub async fn may_read(
        &self,
        agent: &Agent,
        idea: &str,
    ) -> Result<Option<bool>, Box<dyn Error + Send + Sync>> {
        let Some(project) = self
            .catalog
            .project_of(&Subject::Idea(IdeaLink::from(idea)))
            .await?
        else {
            return Ok(None);
        };

        Ok(Some(
            self.authorizer
                .may(agent, Act::Read, project.as_str())
                .await?,
        ))
    }
}

pub struct ReadableBy {
    pub agent: Agent,
    pub readers: Readers,
}

#[async_trait]
impl Sieve for ReadableBy {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let Some(idea) = frame.of.as_deref() else {
            return Ok(false);
        };

        Ok(self
            .readers
            .may_read(&self.agent, idea)
            .await?
            .unwrap_or(false))
    }
}
