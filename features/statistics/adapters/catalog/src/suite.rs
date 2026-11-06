use std::ops::Deref;

use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCount, WordCountCatalog};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions, Tried};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: WordCountCatalog;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Catalog> {
        Committing {
            catalog: self.catalog(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, C> {
    catalog: &'a C,
    transactions: &'a dyn Transactions,
}

fn unreachable(failure: transacting::Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

impl<C: WordCountCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn keep(
        &self,
        scene: &SceneLink,
        project: &ProjectLink,
        words: u64,
        counted_at: OffsetDateTime,
    ) -> Result<Option<u64>, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .catalog
                .keep(transaction, scene, project, words, counted_at)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_scene(
        &self,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget_scene(transaction, scene, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(forgotten)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }
}

impl<C> Deref for Committing<'_, C> {
    type Target = C;

    fn deref(&self) -> &C {
        self.catalog
    }
}

fn a_scene(named: &str) -> SceneLink {
    SceneLink::from(named)
}

fn a_project(named: &str) -> ProjectLink {
    ProjectLink::from(named)
}

fn counted(scene: &str, words: u64) -> WordCount {
    WordCount {
        scene: a_scene(scene),
        words,
    }
}

async fn kept(
    catalog: &Committing<'_, impl WordCountCatalog>,
    scene: &str,
    words: u64,
    seconds: i64,
) {
    catalog
        .keep(
            &a_scene(scene),
            &a_project("project_1"),
            words,
            clock::at(seconds),
        )
        .await
        .expect("keeping should succeed");
}

async fn counts_in(
    catalog: &Committing<'_, impl WordCountCatalog>,
    project: &str,
) -> Vec<WordCount> {
    catalog
        .in_project(&a_project(project))
        .await
        .expect("asking should succeed")
}

async fn forgotten(catalog: &Committing<'_, impl WordCountCatalog>, scene: &str) {
    forgotten_at(catalog, scene, 10_000).await;
}

async fn forgotten_at(catalog: &Committing<'_, impl WordCountCatalog>, scene: &str, seconds: i64) {
    catalog
        .forget_scene(&a_scene(scene), clock::at(seconds))
        .await
        .expect("forgetting should succeed");
}

pub async fn a_project_nobody_counted_has_no_counts(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // expect: no counts for a project nobody counted
    assert!(counts_in(catalog, "project_1").await.is_empty());
}

pub async fn a_kept_count_comes_back_with_its_project(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // when: a count is kept
    kept(catalog, "scene_1", 42, 1_000).await;

    // then: its project has it
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![counted("scene_1", 42)]
    );
}

pub async fn another_project_sees_none_of_them(catalog: &Committing<'_, impl WordCountCatalog>) {
    // given: a count kept for one project
    kept(catalog, "scene_1", 42, 1_000).await;

    // then: another project has none
    assert!(counts_in(catalog, "project_2").await.is_empty());
}

pub async fn counts_come_back_in_one_order_whatever_order_they_were_kept_in(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // when: counts are kept out of order
    kept(catalog, "scene_2", 2, 1_000).await;
    kept(catalog, "scene_3", 3, 1_000).await;
    kept(catalog, "scene_1", 1, 1_000).await;

    // then: they come back ordered by scene
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![
            counted("scene_1", 1),
            counted("scene_2", 2),
            counted("scene_3", 3)
        ]
    );
}

pub async fn a_newer_count_takes_the_place_of_an_older_one(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // given: a count of a scene
    kept(catalog, "scene_1", 42, 1_000).await;

    // when: a newer count of it is kept
    kept(catalog, "scene_1", 50, 2_000).await;

    // then: only the newer one comes back
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![counted("scene_1", 50)]
    );
}

pub async fn a_late_count_does_not_undo_a_newer_one(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // given: a count of a scene
    kept(catalog, "scene_1", 50, 2_000).await;

    // when: an older count of it arrives
    kept(catalog, "scene_1", 42, 1_000).await;

    // then: the newer one stays
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![counted("scene_1", 50)],
        "counts may arrive out of order, and the one counted last is the one that is true"
    );
}

pub async fn a_forgotten_scene_has_no_count(catalog: &Committing<'_, impl WordCountCatalog>) {
    // given: counts of two scenes
    kept(catalog, "scene_1", 42, 1_000).await;
    kept(catalog, "scene_2", 7, 1_000).await;

    // when: one scene is forgotten
    forgotten(catalog, "scene_1").await;

    // then: only the other one's count comes back
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![counted("scene_2", 7)]
    );
}

pub async fn a_count_arriving_after_its_scene_was_forgotten_does_not_bring_it_back(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // given: a forgotten scene
    forgotten(catalog, "scene_1").await;

    // when: a count of it arrives later
    kept(catalog, "scene_1", 42, 3_000).await;

    // then: it has no count
    assert!(
        counts_in(catalog, "project_1").await.is_empty(),
        "a scene counted just before it was deleted can have its count arrive after the deletion"
    );
}

pub async fn keeping_says_what_is_kept(catalog: &Committing<'_, impl WordCountCatalog>) {
    // given: a way of keeping counts of one scene
    let keeping = |words: u64, seconds: i64| async move {
        catalog
            .keep(
                &a_scene("scene_1"),
                &a_project("project_1"),
                words,
                clock::at(seconds),
            )
            .await
            .expect("keeping should succeed")
    };

    // when: a first, a newer and a late count are kept
    let first = keeping(42, 1_000).await;
    let newer = keeping(50, 2_000).await;
    let late = keeping(7, 1_500).await;
    // and: one more after the scene is forgotten
    forgotten(catalog, "scene_1").await;
    let gone = keeping(60, 3_000).await;

    // then: each says the count that is kept
    assert_eq!(
        [first, newer, late, gone],
        [Some(42), Some(50), Some(50), None],
        "a late count leaves the newer one kept, and a forgotten scene keeps none"
    );
}

pub async fn forgetting_what_was_never_counted_is_harmless(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // when: a scene never counted is forgotten twice
    forgotten(catalog, "scene_1").await;
    forgotten(catalog, "scene_1").await;

    // then: there are still no counts
    assert!(counts_in(catalog, "project_1").await.is_empty());
}

async fn forget_long_gone(
    catalog: &Committing<'_, impl WordCountCatalog>,
    before_seconds: i64,
    at_most: u64,
) -> u64 {
    catalog
        .forget_tombstones_laid_before(clock::at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

pub async fn a_forgotten_tombstone_keeps_nothing_out_and_a_later_one_still_does(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // given: a scene forgotten long ago
    forgotten_at(catalog, "scene_long_gone", 1_000).await;
    // and: one forgotten just now
    forgotten_at(catalog, "scene_just_gone", 2_000).await;

    // then: forgetting tombstones laid before now forgets only the old one
    assert_eq!(forget_long_gone(catalog, 2_000, u64::MAX).await, 1);

    // when: counts of both scenes arrive
    kept(catalog, "scene_long_gone", 5, 3_000).await;
    kept(catalog, "scene_just_gone", 6, 3_000).await;
    // then: only the scene whose tombstone was forgotten has one
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![counted("scene_long_gone", 5)],
        "laid at the moment asked about is not before it"
    );
}

pub async fn a_scene_s_tombstone_is_laid_once_when_it_first_went(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // given: a scene forgotten
    forgotten_at(catalog, "scene_1", 1_000).await;

    // when: it is forgotten again later
    forgotten_at(catalog, "scene_1", 5_000).await;

    // then: its tombstone is as old as the first forgetting
    assert_eq!(
        forget_long_gone(catalog, 2_000, u64::MAX).await,
        1,
        "a deletion heard again is the same deletion"
    );
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // given: three scenes forgotten one after another
    for (scene, seconds) in [
        ("scene_newest", 1_002),
        ("scene_oldest", 1_000),
        ("scene_older", 1_001),
    ] {
        forgotten_at(catalog, scene, seconds).await;
    }

    // then: forgetting at most two tombstones forgets two
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 2);

    // when: counts of all three arrive
    for scene in ["scene_newest", "scene_oldest", "scene_older"] {
        kept(catalog, scene, 7, 9_500).await;
    }
    // then: the two oldest scenes have them
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![counted("scene_older", 7), counted("scene_oldest", 7)]
    );
    // and: one tombstone is left to forget
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 1);
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl WordCountCatalog>,
) {
    // given: a kept count
    kept(catalog, "scene_kept", 5, 1_000).await;
    // and: a forgotten scene
    forgotten_at(catalog, "scene_gone", 1_000).await;
    // and: a transaction that keeps, forgets and forgets tombstones
    let mut transaction = catalog.begun().await;
    let uncommitted = catalog.uncommitted();
    uncommitted
        .keep(
            &mut transaction,
            &a_scene("scene_new"),
            &a_project("project_1"),
            3,
            clock::at(2_000),
        )
        .await
        .expect("keeping should succeed");
    uncommitted
        .forget_scene(&mut transaction, &a_scene("scene_kept"), clock::at(2_000))
        .await
        .expect("forgetting should succeed");
    uncommitted
        .forget_tombstones_laid_before(&mut transaction, clock::at(5_000), 10)
        .await
        .expect("forgetting tombstones should succeed");

    // when: the transaction is dropped
    drop(transaction);
    // and: a count of the forgotten scene arrives
    kept(catalog, "scene_gone", 9, 3_000).await;

    // then: only what was there before stands
    assert_eq!(
        counts_in(catalog, "project_1").await,
        vec![counted("scene_kept", 5)],
        "nothing written in the dropped transaction stands, and the tombstone it forgot is still laid"
    );
}

macro_rules! conformance_case {
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

macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::suite::conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::suite::conformance_case!($workbench, a_project_nobody_counted_has_no_counts);
        $crate::suite::conformance_case!($workbench, a_kept_count_comes_back_with_its_project);
        $crate::suite::conformance_case!($workbench, another_project_sees_none_of_them);
        $crate::suite::conformance_case!(
            $workbench,
            counts_come_back_in_one_order_whatever_order_they_were_kept_in
        );
        $crate::suite::conformance_case!($workbench, a_newer_count_takes_the_place_of_an_older_one);
        $crate::suite::conformance_case!($workbench, a_late_count_does_not_undo_a_newer_one);
        $crate::suite::conformance_case!($workbench, a_forgotten_scene_has_no_count);
        $crate::suite::conformance_case!(
            $workbench,
            a_count_arriving_after_its_scene_was_forgotten_does_not_bring_it_back
        );
        $crate::suite::conformance_case!($workbench, keeping_says_what_is_kept);
        $crate::suite::conformance_case!($workbench, forgetting_what_was_never_counted_is_harmless);
        $crate::suite::conformance_case!(
            $workbench,
            a_forgotten_tombstone_keeps_nothing_out_and_a_later_one_still_does
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_scene_s_tombstone_is_laid_once_when_it_first_went
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
    };
}

pub(crate) use {conformance_case, conformance_tests};
