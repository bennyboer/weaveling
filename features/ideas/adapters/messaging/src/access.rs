use access::Role;
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use ideas_core::IdeasCore;
use inbox::KeptHandler;
use members_contract::{
    DISBANDED, FOUNDED, JOINED, LEFT, MembersEventDTO, REMOVED, ROLE_CHANGED, role,
};
use messaging::{Failure, Message};
use projects_contract::{ProjectEventDTO, RESTORED, TRASHED};
use transacting::Transaction;

/// Who is a member of which project in which role, as the members tell it.
pub struct RememberRolesListener {
    ideas: IdeasCore,
}

impl RememberRolesListener {
    pub fn new(ideas: IdeasCore) -> Self {
        Self { ideas }
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
    const NAME: &'static str = "ideas-remember-roles";
    const LISTENS_TO: &'static [&'static str] =
        &[FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED];
    const WHEN_REFUSED: &'static str = "A change to a project's members did not reach the ideas, so who may see, capture and change its ideas is not as it should be.";

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
                    .ideas
                    .remember_disbanded(Some(transaction), &project, version)
                    .await?);
            }
        };

        Ok(self
            .ideas
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
    ideas: IdeasCore,
}

impl RememberTrashListener {
    pub fn new(ideas: IdeasCore) -> Self {
        Self { ideas }
    }
}

#[async_trait]
impl KeptHandler for RememberTrashListener {
    const NAME: &'static str = "ideas-remember-trash";
    const LISTENS_TO: &'static [&'static str] = &[TRASHED, RESTORED];
    const WHEN_REFUSED: &'static str = "A project put in the trash or restored did not reach the ideas, so its ideas may be shown or changed when they should not, or not when they should.";

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
            .ideas
            .remember_trash(
                Some(transaction),
                &told.aggregate.id,
                Version::of(told.aggregate.version),
                in_the_trash,
            )
            .await?)
    }
}
