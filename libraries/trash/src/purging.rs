use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chores::Chore;
use clock::Clock;

use crate::{Trash, Trashed};

pub const PURGED_AT_MOST: u64 = 100;

const PURGED_EVERY: Duration = Duration::from_secs(60 * 60);

/// What a feature does with something whose time in the trash is over: remove it for good, by its kind, and take it out of the trash, in one transaction.
#[async_trait]
pub trait Purge: Send + Sync {
    async fn purge(&self, trashed: &Trashed) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub struct PurgeTheTrash {
    named: &'static str,
    trash: Arc<dyn Trash>,
    purge: Arc<dyn Purge>,
    clock: Arc<dyn Clock>,
}

impl PurgeTheTrash {
    pub fn new(
        named: &'static str,
        trash: Arc<dyn Trash>,
        purge: Arc<dyn Purge>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            named,
            trash,
            purge,
            clock,
        }
    }
}

#[async_trait]
impl Chore for PurgeTheTrash {
    fn named(&self) -> &'static str {
        self.named
    }

    fn every(&self) -> Duration {
        PURGED_EVERY
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let now = self.clock.now();
        let mut handed = HashSet::new();
        let mut purged = 0;

        loop {
            let due = self.trash.due(now, PURGED_AT_MOST).await?;
            let mut refused = None;
            for trashed in &due {
                if !handed.insert(trashed.clone()) {
                    return Err(format!(
                        "the {} {} is still due after it was purged, so its purge left it in the trash",
                        trashed.kind, trashed.id
                    )
                    .into());
                }
                match self.purge.purge(trashed).await {
                    Ok(()) => purged += 1,
                    Err(why) => {
                        tracing::warn!(
                            chore = self.named,
                            kind = %trashed.kind,
                            id = %trashed.id,
                            %why,
                            "something due could not be purged"
                        );
                        refused.get_or_insert(why);
                    }
                }
            }
            if let Some(why) = refused {
                return Err(why);
            }
            if (due.len() as u64) < PURGED_AT_MOST {
                return Ok(purged);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use clock::FixedClock;
    use time::OffsetDateTime;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::InMemoryTrash;
    use crate::suite::at;

    const PROJECT: &str = "project";
    const ACCOUNT: &str = "account";

    struct Purging {
        trash: Arc<InMemoryTrash>,
        transactions: InMemoryTransactions,
        purged: Mutex<Vec<Trashed>>,
        refusing: Option<&'static str>,
        taking_out: bool,
    }

    #[async_trait]
    impl Purge for Purging {
        async fn purge(&self, trashed: &Trashed) -> Result<(), Box<dyn Error + Send + Sync>> {
            if self.refusing == Some(trashed.id.as_str()) {
                return Err(format!("{} could not be removed", trashed.id).into());
            }
            let mut transaction = self.transactions.begin().await?;
            if self.taking_out {
                self.trash
                    .take_out(&mut transaction, &trashed.kind, &trashed.id)
                    .await?;
            }
            transaction.finish().await?;
            self.purged
                .lock()
                .expect("nothing panics holding it")
                .push(trashed.clone());

            Ok(())
        }
    }

    struct Bench {
        trash: Arc<InMemoryTrash>,
        purging: Arc<Purging>,
        chore: PurgeTheTrash,
    }

    fn bench(refusing: Option<&'static str>, taking_out: bool) -> Bench {
        let trash = Arc::new(InMemoryTrash::new());
        let purging = Arc::new(Purging {
            trash: trash.clone(),
            transactions: InMemoryTransactions::new(),
            purged: Mutex::new(Vec::new()),
            refusing,
            taking_out,
        });

        Bench {
            chore: PurgeTheTrash::new(
                "purge-the-projects-trash",
                trash.clone(),
                purging.clone(),
                Arc::new(FixedClock::new(at(30))),
            ),
            trash,
            purging,
        }
    }

    fn trashed(kind: &str, id: &str) -> Trashed {
        Trashed {
            kind: kind.to_owned(),
            id: id.to_owned(),
        }
    }

    impl Bench {
        async fn put_into(&self, kind: &str, id: &str, until: OffsetDateTime) {
            let transactions = InMemoryTransactions::new();
            let mut transaction = transactions
                .begin()
                .await
                .expect("beginning should succeed");
            self.trash
                .put_into(&mut transaction, kind, id, until)
                .await
                .expect("putting into the trash should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        fn purged(&self) -> Vec<Trashed> {
            self.purging
                .purged
                .lock()
                .expect("nothing panics holding it")
                .clone()
        }

        async fn still_due(&self) -> Vec<Trashed> {
            self.trash
                .due(at(1_000), 1_000)
                .await
                .expect("asking what is due should succeed")
        }
    }

    #[tokio::test]
    async fn everything_due_of_every_kind_is_purged_batch_after_batch_and_nothing_before_its_time()
    {
        let bench = bench(None, true);
        let many = PURGED_AT_MOST * 2 + 5;
        for nth in 0..many {
            bench
                .put_into(PROJECT, &format!("project_{nth:04}"), at(29))
                .await;
        }
        bench.put_into(ACCOUNT, "account_1", at(29)).await;
        bench.put_into(PROJECT, "project_later", at(31)).await;

        let purged = bench
            .chore
            .carry_out()
            .await
            .expect("purging should succeed");

        assert_eq!(purged, many + 1);
        assert!(
            bench.purged().contains(&trashed(ACCOUNT, "account_1")),
            "the purge is handed each thing with its kind"
        );
        assert_eq!(
            bench.still_due().await,
            [trashed(PROJECT, "project_later")],
            "only what is due"
        );
    }

    #[tokio::test]
    async fn one_that_cannot_be_purged_stays_and_the_others_are_purged_still() {
        let bench = bench(Some("project_2"), true);
        for id in ["project_1", "project_2", "project_3"] {
            bench.put_into(PROJECT, id, at(29)).await;
        }

        let purged = bench.chore.carry_out().await;

        assert!(
            purged.is_err(),
            "a purge refused is told, so the chore says so"
        );
        assert_eq!(
            bench.purged(),
            [trashed(PROJECT, "project_1"), trashed(PROJECT, "project_3")]
        );
        assert_eq!(
            bench.still_due().await,
            [trashed(PROJECT, "project_2")],
            "tried again next time round"
        );
    }

    #[tokio::test]
    async fn a_purge_that_leaves_its_entry_behind_stops_the_round_rather_than_going_on_for_ever() {
        let bench = bench(None, false);
        for nth in 0..PURGED_AT_MOST {
            bench
                .put_into(PROJECT, &format!("project_{nth:04}"), at(29))
                .await;
        }

        let purged = bench.chore.carry_out().await;

        assert!(purged.is_err());
        assert_eq!(
            bench.purged().len() as u64,
            PURGED_AT_MOST,
            "each handed over once"
        );
    }

    #[test]
    fn the_chore_is_named_as_it_is_wired_and_comes_round_hourly() {
        let bench = bench(None, true);

        assert_eq!(bench.chore.named(), "purge-the-projects-trash");
        assert_eq!(bench.chore.every(), Duration::from_secs(3_600));
    }
}
