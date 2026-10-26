mod publishing;

use async_trait::async_trait;
use messaging::{Failure, Handler, Message};
use projects_contract::EVERY_PROJECT;
use projects_core::{ProjectId, ProjectsCore};
pub use publishing::message_for;

pub struct CatalogueProjectsListener {
    projects: ProjectsCore,
}

impl CatalogueProjectsListener {
    pub fn new(projects: ProjectsCore) -> Self {
        Self { projects }
    }
}

#[async_trait]
impl Handler for CatalogueProjectsListener {
    const NAME: &'static str = "catalogue-project";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_PROJECT];
    const WHEN_REFUSED: &'static str = "A change to a project did not reach your list of projects.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .projects
            .catalogue(&eventpublishing::aggregate_in::<ProjectId>(message)?)
            .await?)
    }
}
