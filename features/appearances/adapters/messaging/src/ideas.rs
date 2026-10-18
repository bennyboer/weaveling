use std::sync::Arc;

use appearances_core::{AppearanceCatalog, IdeaLink, Subject};
use eventpublishing::published_in;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Failure, Handler, Message};

pub struct ForgetDiscardedIdea {
    catalog: Arc<dyn AppearanceCatalog>,
}

impl ForgetDiscardedIdea {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
    }
}

#[async_trait::async_trait]
impl Handler for ForgetDiscardedIdea {
    const NAME: &'static str = "forget-appearances-of-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea is still listed where it appeared.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .catalog
            .forget_subject(&Subject::Idea(IdeaLink::from(discarded.aggregate.id)))
            .await?)
    }
}
