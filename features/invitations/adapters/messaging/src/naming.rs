use async_trait::async_trait;
use eventsourcing::Version;
use inbox::{KeptHandler, Refused};
use invitations_core::{InvitationsCore, ProjectLink, ProjectName};
use messaging::Message;
use projects_contract::{DELETED, ProjectEventDTO, RENAMED, STARTED};
use transacting::Transaction;

pub struct RememberProjectNamesListener {
    invitations: InvitationsCore,
}

impl RememberProjectNamesListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl KeptHandler for RememberProjectNamesListener {
    const NAME: &'static str = "remember-project-names";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, RENAMED, DELETED];
    const WHEN_REFUSED: &'static str = "A project's name, or that it was deleted, did not reach invitations, so an invitation into it may name it wrongly.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = ProjectLink::from(told.aggregate.id);

        match told.event.body {
            ProjectEventDTO::Started { name } | ProjectEventDTO::Renamed { name } => self
                .invitations
                .remember_project_name(
                    Some(transaction),
                    &ProjectName {
                        project,
                        version: Version::of(told.aggregate.version),
                        name,
                    },
                )
                .await
                .map_err(Refused::of),
            ProjectEventDTO::Deleted => self
                .invitations
                .lay_project_tombstone(Some(transaction), &project, message.occurred_at)
                .await
                .map_err(Refused::of),
            ProjectEventDTO::DetailsChanged { .. } | ProjectEventDTO::QuotationChanged { .. } => {
                Err("this tells neither a project's name nor that it was deleted".into())
            }
        }
    }
}
