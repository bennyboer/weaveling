use assets_core::AssetsCore;
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use members_contract::{DISBANDED, FOUNDED, JOINED, LEFT, MembersEventDTO, REMOVED};
use messaging::{Failure, Message};
use transacting::Transaction;

pub struct RememberMembershipsListener {
    assets: AssetsCore,
}

impl RememberMembershipsListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

#[async_trait]
impl KeptHandler for RememberMembershipsListener {
    const NAME: &'static str = "remember-asset-memberships";
    const LISTENS_TO: &'static [&'static str] = &[FOUNDED, JOINED, LEFT, REMOVED, DISBANDED];
    const WHEN_REFUSED: &'static str = "A change to a project's members did not reach the assets, so an account's own pictures may be shown to someone who left, or not to someone who joined.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<MembersEventDTO>(message)?;
        let project = told.aggregate.id;
        let version = Version::of(told.aggregate.version);
        let (account, member) = match told.event.body {
            MembersEventDTO::Founded { owner } => (owner, true),
            MembersEventDTO::Joined { account, .. } => (account, true),
            MembersEventDTO::Left { account } | MembersEventDTO::Removed { account } => {
                (account, false)
            }
            MembersEventDTO::RoleChanged { .. } | MembersEventDTO::Abandoned => return Ok(()),
            MembersEventDTO::Disbanded => {
                return Ok(self
                    .assets
                    .remember_disbanded(Some(transaction), &project, version)
                    .await?);
            }
        };
        Ok(self
            .assets
            .remember_membership(
                Some(transaction),
                &project,
                &AccountId::from(account),
                version,
                member,
            )
            .await?)
    }
}
