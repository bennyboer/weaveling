use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::Version;
use inbox::KeptHandler;
use invitations_core::{InvitationsCore, ProjectLink, ProjectName};
use messaging::{Failure, Message};
use outbox::{Origin, Outbox};
use projects_contract::{DELETED, ProjectEventDTO, RENAMED, RESTORED, STARTED, TRASHED};
use transacting::Transaction;

use crate::sweeping::{SWEEP_PROJECT, SweepProjectDTO};

pub struct RememberProjectNamesListener {
    invitations: InvitationsCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl RememberProjectNamesListener {
    pub fn new(
        invitations: InvitationsCore,
        outbox: Arc<dyn Outbox>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            invitations,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for RememberProjectNamesListener {
    const NAME: &'static str = "remember-project-names";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, RENAMED, TRASHED, RESTORED, DELETED];
    const WHEN_REFUSED: &'static str = "A project's name, or that it was deleted, did not reach invitations, so an invitation into it may name it wrongly.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = ProjectLink::from(told.aggregate.id);

        match told.event.body {
            ProjectEventDTO::Started { name } | ProjectEventDTO::Renamed { name } => Ok(self
                .invitations
                .remember_project_name(
                    Some(transaction),
                    &ProjectName {
                        project,
                        version: Version::of(told.aggregate.version),
                        name,
                    },
                )
                .await?),
            ProjectEventDTO::Deleted => {
                self.invitations
                    .lay_project_tombstone(Some(&mut *transaction), &project, message.occurred_at)
                    .await?;
                self.outbox
                    .enqueue_follow_up_within(
                        transaction,
                        Origin::unversioned(project.as_str(), "project"),
                        message,
                        SWEEP_PROJECT,
                        &SweepProjectDTO {
                            project: project.to_string(),
                        },
                        self.clock.now(),
                    )
                    .await?;

                Ok(())
            }
            ProjectEventDTO::Trashed { .. } | ProjectEventDTO::Restored => Ok(self
                .invitations
                .remember_trash(
                    Some(transaction),
                    &project,
                    Version::of(told.aggregate.version),
                    matches!(told.event.body, ProjectEventDTO::Trashed { .. }),
                )
                .await?),
            ProjectEventDTO::DetailsChanged { .. } | ProjectEventDTO::QuotationChanged { .. } => {
                Err("this tells neither a project's name nor that it was deleted".into())
            }
        }
    }
}
