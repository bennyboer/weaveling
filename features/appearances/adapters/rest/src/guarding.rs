use std::error::Error;

use appearances_core::AppearancesCore;
use async_trait::async_trait;
use eventsourcing::Agent;
use live::Sieve;
use live_contract::FrameDTO;

pub struct ReadableBy {
    pub agent: Agent,
    pub appearances: AppearancesCore,
}

#[async_trait]
impl Sieve for ReadableBy {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let Some(idea) = frame.of.as_deref() else {
            return Ok(false);
        };

        Ok(self.appearances.may_hear(idea, &self.agent).await?)
    }
}
