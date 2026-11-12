use access::Role;
use accounts_contract::{AccountEventDTO, BECAME_OPERATOR, STOPPED_BEING_OPERATOR};
use assets_core::AssetsCore;
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use members_contract::{
    DISBANDED, FOUNDED, JOINED, LEFT, MembersEventDTO, REMOVED, ROLE_CHANGED, role,
};
use messaging::{Failure, Message};
use projects_contract::{ProjectEventDTO, RESTORED, TRASHED};
use transacting::Transaction;

/// Who is a member of which project in which role, as the members tell it.
pub struct RememberRolesListener {
    assets: AssetsCore,
}

impl RememberRolesListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

fn role_named(named: &str) -> Result<Role, Failure> {
    match named {
        role::OWNER => Ok(Role::Owner),
        role::AUTHOR => Ok(Role::Author),
        role::READER => Ok(Role::Reader),
        _ => Err("the members name a role there is none of".into()),
    }
}

#[async_trait]
impl KeptHandler for RememberRolesListener {
    const NAME: &'static str = "assets-remember-roles";
    const LISTENS_TO: &'static [&'static str] =
        &[FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED];
    const WHEN_REFUSED: &'static str = "A change to a project's members did not reach the assets, so who may see and upload its images, or an account's own, is not as it should be.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<MembersEventDTO>(message)?;
        let project = told.aggregate.id;
        let version = Version::of(told.aggregate.version);
        let (account, role) = match told.event.body {
            MembersEventDTO::Founded { owner } => (owner, Some(Role::Owner)),
            MembersEventDTO::Joined { account, role } => (account, Some(role_named(&role)?)),
            MembersEventDTO::RoleChanged { account, to } => (account, Some(role_named(&to)?)),
            MembersEventDTO::Left { account } | MembersEventDTO::Removed { account } => {
                (account, None)
            }
            MembersEventDTO::Abandoned => return Ok(()),
            MembersEventDTO::Disbanded => {
                return Ok(self
                    .assets
                    .remember_disbanded(Some(transaction), &project, version)
                    .await?);
            }
        };

        Ok(self
            .assets
            .remember_role(
                Some(transaction),
                &project,
                &AccountId::from(account),
                version,
                role,
            )
            .await?)
    }
}

/// Which projects are in the trash, as the projects tell it.
pub struct RememberTrashListener {
    assets: AssetsCore,
}

impl RememberTrashListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

#[async_trait]
impl KeptHandler for RememberTrashListener {
    const NAME: &'static str = "assets-remember-trash";
    const LISTENS_TO: &'static [&'static str] = &[TRASHED, RESTORED];
    const WHEN_REFUSED: &'static str = "A project put in the trash or restored did not reach the assets, so its images may be shown when they should not, or not when they should.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let in_the_trash = match told.event.body {
            ProjectEventDTO::Trashed { .. } => true,
            ProjectEventDTO::Restored => false,
            _ => return Err("this tells neither a trashing nor a restoring".into()),
        };

        Ok(self
            .assets
            .remember_trash(
                Some(transaction),
                &told.aggregate.id,
                Version::of(told.aggregate.version),
                in_the_trash,
            )
            .await?)
    }
}

/// Who runs the instance, as the accounts tell it.
pub struct RememberOperatorsListener {
    assets: AssetsCore,
}

impl RememberOperatorsListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

#[async_trait]
impl KeptHandler for RememberOperatorsListener {
    const NAME: &'static str = "assets-remember-operators";
    const LISTENS_TO: &'static [&'static str] = &[
        BECAME_OPERATOR,
        STOPPED_BEING_OPERATOR,
        accounts_contract::DELETED,
    ];
    const WHEN_REFUSED: &'static str = "A change to who runs the instance did not reach the assets, so storage limits may be set by one who may not, or not by one who may.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let operator = match told.event.body {
            AccountEventDTO::BecameOperator => true,
            AccountEventDTO::StoppedBeingOperator | AccountEventDTO::Deleted => false,
            _ => return Err("this tells nothing of who runs the instance".into()),
        };

        Ok(self
            .assets
            .remember_operator(
                Some(transaction),
                &AccountId::from(told.aggregate.id),
                Version::of(told.aggregate.version),
                operator,
            )
            .await?)
    }
}
