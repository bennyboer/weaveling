use async_trait::async_trait;
use eventsourcing::Version;
use invitations_core::{InvitationsCore, ProjectLink, ProjectName};
use messaging::{Failure, Handler, Message};
use projects_contract::{DELETED, ProjectEventDTO, RENAMED, STARTED};

pub struct RememberProjectNamesListener {
    invitations: InvitationsCore,
}

impl RememberProjectNamesListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl Handler for RememberProjectNamesListener {
    const NAME: &'static str = "remember-project-names";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, RENAMED, DELETED];
    const WHEN_REFUSED: &'static str = "A project's name, or that it was deleted, did not reach invitations, so an invitation into it may name it wrongly.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = ProjectLink::from(told.aggregate.id);

        match told.event.body {
            ProjectEventDTO::Started { name } | ProjectEventDTO::Renamed { name } => Ok(self
                .invitations
                .remember_project_name(&ProjectName {
                    project,
                    version: Version::of(told.aggregate.version),
                    name,
                })
                .await?),
            ProjectEventDTO::Deleted => Ok(self
                .invitations
                .lay_project_tombstone(&project, message.occurred_at)
                .await?),
            ProjectEventDTO::DetailsChanged { .. } | ProjectEventDTO::QuotationChanged { .. } => {
                Err("this tells neither a project's name nor that it was deleted".into())
            }
        }
    }
}
