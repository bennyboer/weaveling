use std::ops::Deref;

use transacting::Transactions;

use crate::{Registry, RegistryError};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Registry;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        Committing {
            store: self.store(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, R> {
    store: &'a R,
    transactions: &'a dyn Transactions,
}

impl<R: Registry> Committing<'_, R> {
    pub async fn claim(&self, kind: &str, key: &str, id: &str) -> Result<String, RegistryError> {
        let mut transaction = self
            .transactions
            .begin()
            .await
            .map_err(|failure| RegistryError::Backend(Box::new(failure)))?;
        let held = self.store.claim(&mut transaction, kind, key, id).await?;
        transaction
            .finish()
            .await
            .map_err(|failure| RegistryError::Backend(Box::new(failure)))?;

        Ok(held)
    }
}

impl<R> Deref for Committing<'_, R> {
    type Target = R;

    fn deref(&self) -> &R {
        self.store
    }
}

pub async fn an_unclaimed_key_is_taken_by_whoever_asks(registry: &Committing<'_, impl Registry>) {
    // when: an unclaimed key is claimed
    let held = registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");

    // then: the claimant holds it
    assert_eq!(held, "board_1");
}

pub async fn a_claimed_key_answers_with_who_holds_it(registry: &Committing<'_, impl Registry>) {
    // given: a claimed key
    registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");

    // when: another claims it
    let held = registry
        .claim("board", "project_1", "board_2")
        .await
        .expect("a lost claim is not a failure");

    // then: it is told who holds it
    assert_eq!(
        held, "board_1",
        "the loser is told who won rather than being refused, so it can go and read that one"
    );
}

pub async fn claiming_the_same_key_twice_over_is_stable(registry: &Committing<'_, impl Registry>) {
    // when: one key is claimed four times over
    let mut held = Vec::new();
    for attempt in 1..=4 {
        held.push(
            registry
                .claim("board", "project_1", &format!("board_{attempt}"))
                .await
                .expect("claiming should succeed"),
        );
    }

    // then: every claim is told the first claimant
    assert_eq!(held, vec!["board_1"; 4], "a claim never changes hands");
}

pub async fn each_key_is_claimed_on_its_own(registry: &Committing<'_, impl Registry>) {
    // when: two keys are claimed
    let mine = registry
        .claim("board", "project_mine", "board_1")
        .await
        .expect("claiming should succeed");
    let theirs = registry
        .claim("board", "project_theirs", "board_2")
        .await
        .expect("claiming should succeed");

    // then: each is held by its own claimant
    assert_eq!((mine.as_str(), theirs.as_str()), ("board_1", "board_2"));
}

pub async fn two_kinds_do_not_share_a_key(registry: &Committing<'_, impl Registry>) {
    // when: one key is claimed for two kinds
    let board = registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");
    let outline = registry
        .claim("outline", "project_1", "outline_1")
        .await
        .expect("claiming should succeed");

    // then: each kind is held by its own claimant
    assert_eq!(
        (board.as_str(), outline.as_str()),
        ("board_1", "outline_1"),
        "a project has one board and one outline, and neither claim may shadow the other"
    );
}

pub async fn an_unclaimed_key_has_no_holder(registry: &Committing<'_, impl Registry>) {
    // when: an unclaimed key's holder is asked for
    let held = registry
        .holder("board", "project_1")
        .await
        .expect("asking should succeed");

    // then: there is none
    assert_eq!(held, None);
}

pub async fn a_claimed_key_names_its_holder_without_claiming_anything(
    registry: &Committing<'_, impl Registry>,
) {
    // given: a claimed key
    registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");

    // when: its holder is asked for
    let held = registry
        .holder("board", "project_1")
        .await
        .expect("asking should succeed");

    // then: it is named
    assert_eq!(held.as_deref(), Some("board_1"));
    // and: asking for an unclaimed key claims nothing
    assert_eq!(
        registry
            .holder("board", "project_2")
            .await
            .expect("asking should succeed"),
        None,
        "asking must never claim, or a sweep would start the board it came to discard"
    );
}

pub async fn each_kind_names_its_own_holder(registry: &Committing<'_, impl Registry>) {
    // given: a key claimed for one kind
    registry
        .claim("board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");

    // when: its holder is asked for under another kind
    let outline = registry
        .holder("outline", "project_1")
        .await
        .expect("asking should succeed");

    // then: there is none
    assert_eq!(outline, None);
}

pub async fn a_key_claimed_in_a_dropped_transaction_was_never_claimed(
    registry: &Committing<'_, impl Registry>,
) {
    // given: a key claimed in a transaction
    let mut transaction = registry
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    registry
        .store
        .claim(&mut transaction, "board", "project_1", "board_1")
        .await
        .expect("claiming should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the key has no holder
    assert_eq!(
        registry
            .holder("board", "project_1")
            .await
            .expect("asking should succeed"),
        None
    );
    // and: the next claimant takes it
    assert_eq!(
        registry
            .claim("board", "project_1", "board_2")
            .await
            .expect("claiming should succeed"),
        "board_2",
        "a claim rolled back with what it was claimed for leaves the key to the next"
    );
}

#[macro_export]
macro_rules! registry_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::registry_conformance_case!($workbench, an_unclaimed_key_is_taken_by_whoever_asks);
        $crate::registry_conformance_case!(
            $workbench,
            a_key_claimed_in_a_dropped_transaction_was_never_claimed
        );
        $crate::registry_conformance_case!($workbench, a_claimed_key_answers_with_who_holds_it);
        $crate::registry_conformance_case!($workbench, claiming_the_same_key_twice_over_is_stable);
        $crate::registry_conformance_case!($workbench, each_key_is_claimed_on_its_own);
        $crate::registry_conformance_case!($workbench, two_kinds_do_not_share_a_key);
        $crate::registry_conformance_case!($workbench, an_unclaimed_key_has_no_holder);
        $crate::registry_conformance_case!(
            $workbench,
            a_claimed_key_names_its_holder_without_claiming_anything
        );
        $crate::registry_conformance_case!($workbench, each_kind_names_its_own_holder);
    };
}
