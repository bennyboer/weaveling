use std::sync::Arc;

use assets_contract::OwnerDTO;
use assets_core::{AssetsCore, Owner, ProjectLink};
use async_trait::async_trait;
use clock::Clock;
use eventsourcing::AccountId;
use messaging::{Failure, Handler, Message, WithoutInbox};
use outbox::{Origin, Outbox};
use serde::{Deserialize, Serialize};
use transacting::Transaction;

pub const SWEEP_OWNER: &str = "follow-up.asset.sweep-owner";

/// Asks for every asset of a deleted project or account to be deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepOwnerDTO {
    pub owner: OwnerDTO,
}

/// Queues the sweep of a deleted owner's assets, in the transaction that heard the deletion.
/// The sweep runs as a follow-up once that transaction has committed.
pub(crate) async fn sweep_asked(
    outbox: &Arc<dyn Outbox>,
    clock: &dyn Clock,
    transaction: &mut Transaction,
    heard: &Message,
    owner: OwnerDTO,
) -> Result<(), Failure> {
    let (id, kind) = match &owner {
        OwnerDTO::Project(id) => (id.clone(), "project"),
        OwnerDTO::Account(id) => (id.clone(), "account"),
    };
    outbox
        .enqueue_follow_up_within(
            transaction,
            Origin::unversioned(&id, kind),
            heard,
            SWEEP_OWNER,
            &SweepOwnerDTO { owner },
            clock.now(),
        )
        .await?;

    Ok(())
}

/// Deletes every asset of a deleted project or account, when its sweep is asked for.
pub struct SweepAssetsOfDeletedOwnerListener {
    assets: AssetsCore,
}

impl SweepAssetsOfDeletedOwnerListener {
    pub fn new(assets: AssetsCore) -> Self {
        Self { assets }
    }
}

#[async_trait]
impl Handler for SweepAssetsOfDeletedOwnerListener {
    const NAME: &'static str = "assets-sweep-deleted-owner";
    const LISTENS_TO: &'static [&'static str] = &[SWEEP_OWNER];
    const WHEN_REFUSED: &'static str = "The images of a deleted project or account were not deleted, so they still take up storage.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: SweepOwnerDTO = message.payload_as()?;
        let owner = match asked.owner {
            OwnerDTO::Project(project) => Owner::Project(ProjectLink::from(project.as_str())),
            OwnerDTO::Account(account) => Owner::Account(AccountId::from(account)),
        };
        self.assets.sweep(&owner).await?;

        Ok(())
    }
}
