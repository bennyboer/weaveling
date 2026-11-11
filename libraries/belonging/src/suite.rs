use std::ops::Deref;

use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::{Belongings, BelongingsError};

const PROJECT: &str = "project";

fn at(seconds: i64) -> OffsetDateTime {
    clock::at(seconds)
}

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: Belongings;

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

pub struct Committing<'a, B> {
    store: &'a B,
    transactions: &'a dyn Transactions,
}

fn unreachable(failure: transacting::Failure) -> BelongingsError {
    BelongingsError::Backend(Box::new(failure))
}

impl<B: Belongings> Committing<'_, B> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &B {
        self.store
    }

    pub async fn remember(
        &self,
        kind: &str,
        id: &str,
        owner_kind: &str,
        owner: &str,
    ) -> Result<(), BelongingsError> {
        let mut transaction = self.transactions.begin().await.map_err(unreachable)?;
        self.store
            .remember(&mut transaction, kind, id, owner_kind, owner)
            .await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn forget(&self, kind: &str, id: &str) -> Result<(), BelongingsError> {
        self.forget_at(kind, id, at(1_000)).await
    }

    pub async fn forget_at(
        &self,
        kind: &str,
        id: &str,
        laid: OffsetDateTime,
    ) -> Result<(), BelongingsError> {
        let mut transaction = self.transactions.begin().await.map_err(unreachable)?;
        self.store.forget(&mut transaction, kind, id, laid).await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, BelongingsError> {
        let mut transaction = self.transactions.begin().await.map_err(unreachable)?;
        let forgotten = self
            .store
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await.map_err(unreachable)?;

        Ok(forgotten)
    }

    pub async fn owner(&self, kind: &str, id: &str, owner_kind: &str) -> Option<String> {
        self.store
            .owner_of(kind, id, owner_kind)
            .await
            .expect("asking should succeed")
    }
}

impl<B> Deref for Committing<'_, B> {
    type Target = B;

    fn deref(&self) -> &B {
        self.store
    }
}

pub async fn what_was_never_remembered_belongs_to_no_owner_known(
    belongings: &Committing<'_, impl Belongings>,
) {
    // expect: an idea never heard of is of no project known
    assert_eq!(belongings.owner("idea", "idea_1", PROJECT).await, None);
}

pub async fn what_is_remembered_belongs_to_its_owner(belongings: &Committing<'_, impl Belongings>) {
    // when: two ideas are remembered in two projects
    for (idea, project) in [("idea_1", "project_1"), ("idea_2", "project_2")] {
        belongings
            .remember("idea", idea, PROJECT, project)
            .await
            .expect("remembering should succeed");
    }

    // then: each belongs to its own
    assert_eq!(
        belongings.owner("idea", "idea_1", PROJECT).await.as_deref(),
        Some("project_1")
    );
    assert_eq!(
        belongings.owner("idea", "idea_2", PROJECT).await.as_deref(),
        Some("project_2")
    );
}

pub async fn remembering_again_keeps_the_last(belongings: &Committing<'_, impl Belongings>) {
    // when: an idea is remembered twice in one project and then in another
    for project in ["project_1", "project_1", "project_2"] {
        belongings
            .remember("idea", "idea_1", PROJECT, project)
            .await
            .expect("remembering again is no failure");
    }

    // then: the last is kept
    assert_eq!(
        belongings.owner("idea", "idea_1", PROJECT).await.as_deref(),
        Some("project_2")
    );
}

pub async fn two_kinds_do_not_share_an_id(belongings: &Committing<'_, impl Belongings>) {
    // when: an idea and a scene sharing an id are remembered in two projects
    belongings
        .remember("idea", "same_1", PROJECT, "project_1")
        .await
        .expect("remembering should succeed");
    belongings
        .remember("scene", "same_1", PROJECT, "project_2")
        .await
        .expect("remembering should succeed");

    // then: each belongs to its own
    assert_eq!(
        belongings.owner("idea", "same_1", PROJECT).await.as_deref(),
        Some("project_1")
    );
    assert_eq!(
        belongings
            .owner("scene", "same_1", PROJECT)
            .await
            .as_deref(),
        Some("project_2")
    );
}

pub async fn a_thing_has_an_owner_of_each_kind(belongings: &Committing<'_, impl Belongings>) {
    // when: an idea is remembered in a project and on a board
    belongings
        .remember("idea", "idea_1", PROJECT, "project_1")
        .await
        .expect("remembering should succeed");
    belongings
        .remember("idea", "idea_1", "board", "board_1")
        .await
        .expect("remembering should succeed");

    // then: each owner is kept beside the other
    assert_eq!(
        belongings.owner("idea", "idea_1", PROJECT).await.as_deref(),
        Some("project_1")
    );
    assert_eq!(
        belongings.owner("idea", "idea_1", "board").await.as_deref(),
        Some("board_1")
    );
    // and: an owner of a kind never remembered is none
    assert_eq!(belongings.owner("idea", "idea_1", "outline").await, None);
}

pub async fn what_is_forgotten_belongs_to_no_owner_of_any_kind_and_the_rest_stays(
    belongings: &Committing<'_, impl Belongings>,
) {
    // given: two ideas remembered, the first with owners of two kinds
    for idea in ["idea_gone", "idea_kept"] {
        belongings
            .remember("idea", idea, PROJECT, "project_1")
            .await
            .expect("remembering should succeed");
    }
    belongings
        .remember("idea", "idea_gone", "board", "board_1")
        .await
        .expect("remembering should succeed");

    // when: the first is forgotten
    belongings
        .forget("idea", "idea_gone")
        .await
        .expect("forgetting should succeed");

    // then: it has no owner of either kind
    assert_eq!(belongings.owner("idea", "idea_gone", PROJECT).await, None);
    assert_eq!(belongings.owner("idea", "idea_gone", "board").await, None);
    // and: the other stays
    assert_eq!(
        belongings
            .owner("idea", "idea_kept", PROJECT)
            .await
            .as_deref(),
        Some("project_1")
    );
}

pub async fn forgetting_what_was_never_remembered_is_harmless(
    belongings: &Committing<'_, impl Belongings>,
) {
    // expect: forgetting an idea never heard of succeeds
    belongings
        .forget("idea", "idea_never")
        .await
        .expect("forgetting nothing is not a failure");
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    belongings: &Committing<'_, impl Belongings>,
) {
    // given: an idea remembered
    belongings
        .remember("idea", "idea_kept", PROJECT, "project_1")
        .await
        .expect("remembering should succeed");
    // and: a transaction remembering another and forgetting the first
    let mut transaction = belongings.begun().await;
    let store = belongings.uncommitted();
    store
        .remember(&mut transaction, "idea", "idea_new", PROJECT, "project_1")
        .await
        .expect("remembering should succeed");
    store
        .forget(&mut transaction, "idea", "idea_kept", at(1_000))
        .await
        .expect("forgetting should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the first is still remembered and the other never was
    assert_eq!(
        belongings
            .owner("idea", "idea_kept", PROJECT)
            .await
            .as_deref(),
        Some("project_1")
    );
    assert_eq!(belongings.owner("idea", "idea_new", PROJECT).await, None);
}

pub async fn what_is_heard_of_again_once_forgotten_is_not_remembered(
    belongings: &Committing<'_, impl Belongings>,
) {
    // given: an idea remembered and then forgotten
    belongings
        .remember("idea", "idea_gone", PROJECT, "project_1")
        .await
        .expect("remembering should succeed");
    belongings
        .forget("idea", "idea_gone")
        .await
        .expect("forgetting should succeed");

    // when: its capture is heard again, late
    belongings
        .remember("idea", "idea_gone", PROJECT, "project_1")
        .await
        .expect("remembering should succeed");

    // then: it belongs to no owner known
    assert_eq!(
        belongings.owner("idea", "idea_gone", PROJECT).await,
        None,
        "a thing gone is not brought back by what is heard of it late"
    );
    // and: a scene of the same id is still remembered, being another kind
    belongings
        .remember("scene", "idea_gone", PROJECT, "project_1")
        .await
        .expect("remembering should succeed");
    assert_eq!(
        belongings
            .owner("scene", "idea_gone", PROJECT)
            .await
            .as_deref(),
        Some("project_1")
    );
}

pub async fn old_tombstones_are_forgotten_oldest_first_and_the_younger_kept(
    belongings: &Committing<'_, impl Belongings>,
) {
    // given: three ideas forgotten at one, two and five
    for (idea, laid) in [("idea_1", 1_000), ("idea_2", 2_000), ("idea_5", 5_000)] {
        belongings
            .forget_at("idea", idea, at(laid))
            .await
            .expect("forgetting should succeed");
    }

    // when: one tombstone laid before three is forgotten, then the rest
    let first = belongings
        .forget_tombstones_laid_before(at(3_000), 1)
        .await
        .expect("forgetting should succeed");
    let rest = belongings
        .forget_tombstones_laid_before(at(3_000), 100)
        .await
        .expect("forgetting should succeed");

    // then: one each time, the ones before three
    assert_eq!((first, rest), (1, 1));
    // and: those ideas are remembered again once heard of, the younger one not
    for idea in ["idea_1", "idea_2", "idea_5"] {
        belongings
            .remember("idea", idea, PROJECT, "project_1")
            .await
            .expect("remembering should succeed");
    }
    assert_eq!(
        [
            belongings.owner("idea", "idea_1", PROJECT).await.is_some(),
            belongings.owner("idea", "idea_2", PROJECT).await.is_some(),
            belongings.owner("idea", "idea_5", PROJECT).await.is_some(),
        ],
        [true, true, false]
    );
}

#[macro_export]
macro_rules! belongings_conformance_case {
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
        $crate::belongings_conformance_case!(
            $workbench,
            what_was_never_remembered_belongs_to_no_owner_known
        );
        $crate::belongings_conformance_case!($workbench, what_is_remembered_belongs_to_its_owner);
        $crate::belongings_conformance_case!($workbench, remembering_again_keeps_the_last);
        $crate::belongings_conformance_case!($workbench, two_kinds_do_not_share_an_id);
        $crate::belongings_conformance_case!($workbench, a_thing_has_an_owner_of_each_kind);
        $crate::belongings_conformance_case!(
            $workbench,
            what_is_forgotten_belongs_to_no_owner_of_any_kind_and_the_rest_stays
        );
        $crate::belongings_conformance_case!(
            $workbench,
            forgetting_what_was_never_remembered_is_harmless
        );
        $crate::belongings_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::belongings_conformance_case!(
            $workbench,
            what_is_heard_of_again_once_forgotten_is_not_remembered
        );
        $crate::belongings_conformance_case!(
            $workbench,
            old_tombstones_are_forgotten_oldest_first_and_the_younger_kept
        );
    };
}
