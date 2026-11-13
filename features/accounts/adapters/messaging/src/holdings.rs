use accounts_core::{AccountId, AccountsCore, InProject};
use async_trait::async_trait;
use eventsourcing::Version;
use inbox::KeptHandler;
use members_contract::{
    DISBANDED, FOUNDED, JOINED, LEFT, MembersEventDTO, REMOVED, ROLE_CHANGED, role,
};
use messaging::{Failure, Message};
use transacting::Transaction;

pub struct RememberProjectMembersListener {
    accounts: AccountsCore,
}

impl RememberProjectMembersListener {
    pub fn new(accounts: AccountsCore) -> Self {
        Self { accounts }
    }
}

#[async_trait]
impl KeptHandler for RememberProjectMembersListener {
    const NAME: &'static str = "remember-project-members";
    const LISTENS_TO: &'static [&'static str] =
        &[FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED];
    const WHEN_REFUSED: &'static str = "A change to a project's members did not reach the accounts feature. An account may be deleted while it is still the last owner of a project that others are in. Or an account may be kept from being deleted although it is not such an owner.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<MembersEventDTO>(message)?;
        let project = told.aggregate.id;
        let version = Version::of(told.aggregate.version);
        let (account, held) = match told.event.body {
            MembersEventDTO::Founded { owner } => (owner, Some(InProject::Owner)),
            MembersEventDTO::Joined { account, role } => (account, Some(held_as(&role))),
            MembersEventDTO::RoleChanged { account, to } => (account, Some(held_as(&to))),
            MembersEventDTO::Left { account } | MembersEventDTO::Removed { account } => {
                (account, None)
            }
            MembersEventDTO::Abandoned => {
                return Err("an abandoning names nobody, as the removal before it did".into());
            }
            MembersEventDTO::Disbanded => {
                return Ok(self
                    .accounts
                    .remember_disbanded(Some(transaction), &project, version)
                    .await?);
            }
        };
        let account: AccountId = account
            .parse()
            .map_err(|_| "the members name an account by no account's id")?;

        Ok(self
            .accounts
            .remember_holding(Some(transaction), &project, &account, version, held)
            .await?)
    }
}

fn held_as(named: &str) -> InProject {
    if named == role::OWNER {
        InProject::Owner
    } else {
        InProject::Member
    }
}
