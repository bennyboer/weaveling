use std::sync::Arc;

use accounts_contract::{AccountEventDTO, DELETED, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use clock::Clock;
use eventsourcing::AccountId;
use feedback_core::FeedbackCore;
use inbox::KeptHandler;
use messaging::{Failure, Handler, Message, WithoutInbox};
use outbox::{Origin, Outbox};
use serde::{Deserialize, Serialize};
use transacting::Transaction;

pub const DROP_ACCOUNT_DRAFTS: &str = "follow-up.feedback.drop-account-drafts";

/// A deleted account whose drafts are still to be dropped.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DropAccountDraftsDTO {
    pub account: String,
}

/// Hears the names accounts sign up with, so feedback names whoever gave it.
/// Once an account is deleted, it forgets its name and asks for its drafts to be dropped, in one transaction.
pub struct RememberAccountNamesListener {
    feedback: FeedbackCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl RememberAccountNamesListener {
    pub fn new(feedback: FeedbackCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            feedback,
            outbox,
            clock,
        }
    }
}

#[async_trait::async_trait]
impl KeptHandler for RememberAccountNamesListener {
    const NAME: &'static str = "feedback-remember-account-names";
    const LISTENS_TO: &'static [&'static str] = &[SIGNED_UP, SIGNED_UP_BY_INVITATION, DELETED];
    const WHEN_REFUSED: &'static str = "An account's sign-up or deletion did not reach the feedback feature. Its feedback may name nobody who gave it, or still name a deleted account.";

    /// Keeps the name an account signed up with, or forgets it once the account is deleted.
    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let account = AccountId::from(told.aggregate.id);

        match told.event.body {
            AccountEventDTO::SignedUp { personal }
            | AccountEventDTO::SignedUpByInvitation { personal } => Ok(self
                .feedback
                .name_heard(Some(transaction), &account, &personal.name)
                .await?),
            AccountEventDTO::Deleted => {
                self.feedback
                    .account_gone(Some(transaction), &account, message.occurred_at)
                    .await?;
                self.outbox
                    .enqueue_follow_up_within(
                        transaction,
                        Origin {
                            aggregate: &account.to_string(),
                            kind: "account",
                            version: Some(told.aggregate.version),
                        },
                        message,
                        DROP_ACCOUNT_DRAFTS,
                        &DropAccountDraftsDTO {
                            account: account.to_string(),
                        },
                        self.clock.now(),
                    )
                    .await?;

                Ok(())
            }
            _ => Err("this tells neither a sign-up nor an account deleted".into()),
        }
    }
}

/// Drops the drafts a deleted account left, as nobody can submit them any more; what it submitted stays.
pub struct DropDraftsOfDeletedAccountListener {
    feedback: FeedbackCore,
}

impl DropDraftsOfDeletedAccountListener {
    pub fn new(feedback: FeedbackCore) -> Self {
        Self { feedback }
    }
}

#[async_trait::async_trait]
impl Handler for DropDraftsOfDeletedAccountListener {
    const NAME: &'static str = "feedback-drop-drafts-of-deleted-account";
    const LISTENS_TO: &'static [&'static str] = &[DROP_ACCOUNT_DRAFTS];
    const WHEN_REFUSED: &'static str = "The drafts of a deleted account were not dropped. Their words stay stored, though nobody can submit or find them any more.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: DropAccountDraftsDTO = message.payload_as()?;

        Ok(self
            .feedback
            .drop_drafts_of(&AccountId::from(asked.account))
            .await?)
    }
}
