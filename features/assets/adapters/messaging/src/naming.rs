use accounts_contract::{AccountEventDTO, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use assets_core::{AssetsCore, Person, ProjectLink};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use messaging::{Failure, Message};
use projects_contract::{ProjectEventDTO, RENAMED, STARTED};
use transacting::Transaction;

pub struct RememberPeopleListener {
    assets: AssetsCore,
}

impl RememberPeopleListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
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
    const WHEN_REFUSED: &'static str = "An account signed up or deleted did not reach the assets, so the operators may not find its storage by its name and address, or still find a deleted account's.";

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
            AccountEventDTO::Deleted => Ok(self
                .assets
                .forget_person(Some(transaction), &account, version)
                .await?),
            _ => Err("this tells neither a sign-up nor an account deleted".into()),
        }
    }
}

pub struct RememberProjectNamesListener {
    assets: AssetsCore,
}

impl RememberProjectNamesListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

#[async_trait]
impl KeptHandler for RememberProjectNamesListener {
    const NAME: &'static str = "assets-remember-project-names";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, RENAMED, projects_contract::DELETED];
    const WHEN_REFUSED: &'static str = "A project's name, or that it was deleted, did not reach the assets, so the operators may not find its storage by its name, or still find a deleted project's.";

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
            ProjectEventDTO::Deleted => Ok(self
                .assets
                .forget_project_name(Some(transaction), &project, version)
                .await?),
            _ => Err("this tells neither a project's name nor that it was deleted".into()),
        }
    }
}
