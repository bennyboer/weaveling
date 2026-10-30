use accounts_contract::{AccountEventDTO, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use messaging::{Failure, Handler, Message};
use notifications_core::{NotificationsCore, Person, ProjectLink, ProjectName};
use projects_contract::{DELETED, ProjectEventDTO, RENAMED, STARTED};

pub struct RememberWhoSignedUpListener {
    notifications: NotificationsCore,
}

impl RememberWhoSignedUpListener {
    pub fn new(notifications: NotificationsCore) -> Self {
        Self { notifications }
    }
}

#[async_trait]
impl Handler for RememberWhoSignedUpListener {
    const NAME: &'static str = "notifications-remember-who-signed-up";
    const LISTENS_TO: &'static [&'static str] = &[SIGNED_UP, SIGNED_UP_BY_INVITATION];
    const WHEN_REFUSED: &'static str =
        "Someone who signed up is not known to notifications, so a notice cannot name them.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let (AccountEventDTO::SignedUp { personal }
        | AccountEventDTO::SignedUpByInvitation { personal }) = told.event.body
        else {
            return Err("this is no sign-up".into());
        };

        Ok(self
            .notifications
            .remember_person(&Person {
                account: AccountId::from(told.aggregate.id),
                name: personal.name,
            })
            .await?)
    }
}

pub struct RememberProjectNamesListener {
    notifications: NotificationsCore,
}

impl RememberProjectNamesListener {
    pub fn new(notifications: NotificationsCore) -> Self {
        Self { notifications }
    }
}

#[async_trait]
impl Handler for RememberProjectNamesListener {
    const NAME: &'static str = "notifications-remember-project-names";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, RENAMED, DELETED];
    const WHEN_REFUSED: &'static str = "A project's name, or that it was deleted, did not reach notifications, so a notice may name it wrongly or stay listed.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = ProjectLink::from(told.aggregate.id.as_str());

        match told.event.body {
            ProjectEventDTO::Started { name } | ProjectEventDTO::Renamed { name } => Ok(self
                .notifications
                .remember_project_name(&ProjectName {
                    project,
                    version: Version::of(told.aggregate.version),
                    name,
                })
                .await?),
            ProjectEventDTO::Deleted => Ok(self
                .notifications
                .lay_project_tombstone(&project, message.occurred_at)
                .await?),
            ProjectEventDTO::DetailsChanged { .. } | ProjectEventDTO::QuotationChanged { .. } => {
                Err("this tells neither a project's name nor that it was deleted".into())
            }
        }
    }
}
