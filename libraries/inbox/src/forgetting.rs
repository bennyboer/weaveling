use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use chores::Tombstones;
use time::OffsetDateTime;
use transacting::Transactions;

use crate::{Failed, Inbox};

pub struct HeardMessages {
    inbox: Arc<dyn Inbox>,
    transactions: Arc<dyn Transactions>,
}

impl HeardMessages {
    pub fn of(inbox: Arc<dyn Inbox>, transactions: Arc<dyn Transactions>) -> Self {
        Self {
            inbox,
            transactions,
        }
    }
}

#[async_trait]
impl Tombstones for HeardMessages {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, Failed>(
                self.inbox
                    .forget_heard_before(transaction, before, at_most)
                    .await?,
            )
        })
        .map_err(|failed| failed.0)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chores::{Chore, ForgetOldTombstones, TOMBSTONES_KEPT_FOR};
    use clock::FixedClock;
    use messaging::{ListenerName, MessageId};
    use time::{Duration, OffsetDateTime};
    use transacting::{InMemoryTransactions, Transactions};

    use super::HeardMessages;
    use crate::{InMemoryInbox, Inbox};

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_chore_forgets_only_what_was_heard_longer_ago_than_tombstones_are_kept() {
        // given: a message heard longer ago than tombstones are kept, and one heard since
        let clock = Arc::new(FixedClock::new(NOW));
        let inbox = Arc::new(InMemoryInbox::new(clock.clone()));
        let transactions = Arc::new(InMemoryTransactions::new());
        let listener = ListenerName::parse("note-role-changed").expect("a plain name is fine");
        let (long_ago, lately) = (MessageId::generate(NOW), MessageId::generate(NOW));
        for (heard, message) in [
            (NOW - TOMBSTONES_KEPT_FOR - Duration::seconds(1), long_ago),
            (NOW - TOMBSTONES_KEPT_FOR + Duration::seconds(1), lately),
        ] {
            clock.set(heard);
            let mut transaction = transactions
                .begin()
                .await
                .expect("beginning should succeed");
            inbox
                .heard_before(&mut transaction, &listener, message)
                .await
                .expect("hearing should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }
        // and: the chore that forgets old hearings
        clock.set(NOW);
        let chore = ForgetOldTombstones::new(
            "forget-old-hearings",
            Arc::new(HeardMessages::of(inbox.clone(), transactions.clone())),
            clock.clone(),
        );

        // when: it is carried out
        let forgotten = chore.carry_out().await.expect("forgetting should succeed");
        // and: both are heard once more
        let mut transaction = transactions
            .begin()
            .await
            .expect("beginning should succeed");
        let heard = (
            inbox
                .heard_before(&mut transaction, &listener, long_ago)
                .await
                .expect("hearing should succeed"),
            inbox
                .heard_before(&mut transaction, &listener, lately)
                .await
                .expect("hearing should succeed"),
        );

        // then: one is forgotten
        assert_eq!(forgotten, 1);
        // and: it is the one heard long ago
        assert_eq!(heard, (false, true), "and it was committed");
    }
}
