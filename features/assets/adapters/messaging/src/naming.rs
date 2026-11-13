use std::sync::Arc;

use accounts_contract::{AccountEventDTO, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use assets_contract::OwnerDTO;
use assets_core::{AssetsCore, Person, ProjectLink};
use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use messaging::{Failure, Message};
use outbox::Outbox;
use projects_contract::{ProjectEventDTO, RENAMED, STARTED};
use transacting::Transaction;

use crate::sweeping;

/// Stores the names and emails of accounts that sign up.
/// When an account is deleted, it removes them and asks for the account's own assets to be swept.
pub struct RememberPeopleListener {
    assets: AssetsCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl RememberPeopleListener {
    pub fn new(assets: AssetsCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            assets,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for RememberPeopleListener {
    const NAME: &'static str = "assets-remember-people";
    const LISTENS_TO: &'static [&'static str] = &[
        SIGNED_UP,
        SIGNED_UP_BY_INVITATION,
        accounts_contract::DELETED,
    ];
    const WHEN_REFUSED: &'static str = "An account's sign-up or deletion did not reach the assets feature. The storage overview may not show the account's name and address, or may still show a deleted account.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let account = AccountId::from(told.aggregate.id);
        let version = Version::of(told.aggregate.version);

        match told.event.body {
            AccountEventDTO::SignedUp { personal }
            | AccountEventDTO::SignedUpByInvitation { personal } => Ok(self
                .assets
                .remember_person(
                    Some(transaction),
                    &Person {
                        account,
                        name: personal.name,
                        email: personal.email,
                    },
                    version,
                )
                .await?),
            AccountEventDTO::Deleted => {
                self.assets
                    .forget_person(Some(&mut *transaction), &account, version)
                    .await?;

                sweeping::sweep_asked(
                    &self.outbox,
                    &*self.clock,
                    transaction,
                    message,
                    OwnerDTO::Account(account.to_string()),
                )
                .await
            }
            _ => Err("this tells neither a sign-up nor an account deleted".into()),
        }
    }
}

/// Stores the names of projects as they are started and renamed.
/// When a project is deleted, it removes the name and asks for the project's assets to be swept.
pub struct RememberProjectNamesListener {
    assets: AssetsCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl RememberProjectNamesListener {
    pub fn new(assets: AssetsCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            assets,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for RememberProjectNamesListener {
    const NAME: &'static str = "assets-remember-project-names";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, RENAMED, projects_contract::DELETED];
    const WHEN_REFUSED: &'static str = "A project's new name or its deletion did not reach the assets feature. The storage overview may show an old name, or still show a deleted project.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = ProjectLink::from(told.aggregate.id.as_str());
        let version = Version::of(told.aggregate.version);

        match told.event.body {
            ProjectEventDTO::Started { name } | ProjectEventDTO::Renamed { name } => Ok(self
                .assets
                .remember_project_name(Some(transaction), &project, version, &name)
                .await?),
            ProjectEventDTO::Deleted => {
                self.assets
                    .forget_project_name(Some(&mut *transaction), &project, version)
                    .await?;

                sweeping::sweep_asked(
                    &self.outbox,
                    &*self.clock,
                    transaction,
                    message,
                    OwnerDTO::Project(project.to_string()),
                )
                .await
            }
            _ => Err("this tells neither a project's name nor that it was deleted".into()),
        }
    }
}
