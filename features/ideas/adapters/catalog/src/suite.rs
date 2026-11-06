use std::ops::Deref;

use eventsourcing::Version;
use ideas_core::{CatalogError, IdeaCatalog, IdeaId, IdeaSummary, IdeaTitle, ProjectLink};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions, Tried};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: IdeaCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        Committing {
            catalog: self.store(),
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

impl<C: IdeaCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn remember(&self, summary: &IdeaSummary) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, summary)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget(&self, id: &IdeaId, at: OffsetDateTime) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, id, at)
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
            let done = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn lay_project_tombstone(
        &self,
        project: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .lay_project_tombstone(transaction, &ProjectLink::from(project), at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn has_project_tombstone(&self, project: &str) -> bool {
        let mut transaction = self.begun().await;
        let buried = self
            .catalog
            .has_project_tombstone_within(&mut transaction, &ProjectLink::from(project))
            .await
            .expect("asking should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        buried
    }
}

impl<C> Deref for Committing<'_, C> {
    type Target = C;

    fn deref(&self) -> &C {
        self.catalog
    }
}

pub fn a_summary(id: IdeaId, project: &str, title: &str) -> IdeaSummary {
    IdeaSummary {
        id,
        version: Version::of(1),
        project: ProjectLink::from(project),
        title: IdeaTitle::new(title).expect("a plain title is fine"),
    }
}

pub async fn a_remembered_idea_is_listed_in_its_project(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a summary of an idea
    let summary = a_summary(IdeaId::generate(clock::at(1_000)), "project_1", "The Loom");

    // when: it is remembered
    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    // then: its project lists it
    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![summary]);
}

pub async fn a_project_nobody_wrote_in_lists_nothing(catalog: &Committing<'_, impl IdeaCatalog>) {
    // when: a project nobody wrote in is listed
    let found = catalog
        .in_project(&ProjectLink::from("project_empty"))
        .await
        .expect("listing an unknown project should not fail");

    // then: nothing is found
    assert!(found.is_empty());
}

pub async fn ideas_of_other_projects_are_not_listed(catalog: &Committing<'_, impl IdeaCatalog>) {
    // given: an idea remembered in one project
    catalog
        .remember(&a_summary(
            IdeaId::generate(clock::at(1_000)),
            "project_mine",
            "Mine",
        ))
        .await
        .expect("remembering should succeed");
    // and: one in another
    catalog
        .remember(&a_summary(
            IdeaId::generate(clock::at(1_000)),
            "project_theirs",
            "Theirs",
        ))
        .await
        .expect("remembering should succeed");

    // when: the first project is listed
    let found = catalog
        .in_project(&ProjectLink::from("project_mine"))
        .await
        .expect("listing should succeed");

    // then: only its own idea is found
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title.as_str(), "Mine");
}

pub async fn remembering_the_same_idea_again_replaces_what_was_there(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a remembered idea
    let id = IdeaId::generate(clock::at(1_000));
    catalog
        .remember(&a_summary(id, "project_1", "The Loom"))
        .await
        .expect("remembering should succeed");

    // when: a later summary of it is remembered
    let mut later = a_summary(id, "project_1", "The Silent Loom");
    later.version = Version::of(2);
    catalog
        .remember(&later)
        .await
        .expect("remembering should succeed");

    // then: only the later summary is listed
    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![later], "a catalog holds one row per idea");
}

pub async fn a_forgotten_idea_is_no_longer_listed(catalog: &Committing<'_, impl IdeaCatalog>) {
    // given: a remembered idea
    let id = IdeaId::generate(clock::at(1_000));
    catalog
        .remember(&a_summary(id, "project_1", "The Loom"))
        .await
        .expect("remembering should succeed");

    // when: it is forgotten
    catalog
        .forget(&id, clock::at(5_000))
        .await
        .expect("forgetting should succeed");

    // then: it is no longer listed
    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

pub async fn forgetting_something_never_remembered_is_harmless(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // expect: forgetting an idea never remembered is no failure
    catalog
        .forget(&IdeaId::generate(clock::at(1_000)), clock::at(5_000))
        .await
        .expect("forgetting an unknown idea should not fail");
}

pub async fn the_newest_idea_is_listed_first(catalog: &Committing<'_, impl IdeaCatalog>) {
    // given: ideas captured one after another
    let captured: Vec<IdeaId> = (1..=6)
        .map(|nth| IdeaId::generate(clock::at(nth * 1_000)))
        .collect();

    for id in &captured {
        catalog
            .remember(&a_summary(*id, "project_1", "An idea"))
            .await
            .expect("remembering should succeed");
    }

    // when: their project is listed
    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    // then: the newest comes first
    assert_eq!(
        found.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        captured.into_iter().rev().collect::<Vec<_>>(),
        "the idea an author had most recently should be the first they see"
    );
}

pub async fn a_batch_starts_at_the_oldest_idea(catalog: &Committing<'_, impl IdeaCatalog>) {
    // given: ideas captured one after another
    let captured: Vec<IdeaId> = (1..=5)
        .map(|nth| IdeaId::generate(clock::at(nth * 1_000)))
        .collect();
    for id in &captured {
        catalog
            .remember(&a_summary(*id, "project_1", "An idea"))
            .await
            .expect("remembering should succeed");
    }

    // when: a first batch is taken
    let batch = catalog
        .in_project_after(&ProjectLink::from("project_1"), None, 2)
        .await
        .expect("listing should succeed");

    // then: it starts at the oldest
    assert_eq!(
        batch.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        captured[..2].to_vec(),
        "a sweep walks forward from the oldest, so the cursor it carries only ever grows"
    );
}

pub async fn a_batch_after_a_cursor_takes_what_comes_next(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: ideas captured one after another
    let captured: Vec<IdeaId> = (1..=5)
        .map(|nth| IdeaId::generate(clock::at(nth * 1_000)))
        .collect();
    for id in &captured {
        catalog
            .remember(&a_summary(*id, "project_1", "An idea"))
            .await
            .expect("remembering should succeed");
    }

    // when: a batch is taken after one of them
    let batch = catalog
        .in_project_after(&ProjectLink::from("project_1"), Some(captured[1]), 2)
        .await
        .expect("listing should succeed");

    // then: it holds the ones that come next
    assert_eq!(
        batch.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        captured[2..4].to_vec()
    );
}

pub async fn a_batch_past_the_end_is_empty(catalog: &Committing<'_, impl IdeaCatalog>) {
    // given: a single remembered idea
    let only = IdeaId::generate(clock::at(1_000));
    catalog
        .remember(&a_summary(only, "project_1", "An idea"))
        .await
        .expect("remembering should succeed");

    // when: a batch is taken after it
    let batch = catalog
        .in_project_after(&ProjectLink::from("project_1"), Some(only), 8)
        .await
        .expect("listing should succeed");

    // then: the batch is empty
    assert!(
        batch.is_empty(),
        "an empty batch is how a sweep learns it is finished, so it must not wrap around"
    );
}

pub async fn a_batch_stays_inside_its_project(catalog: &Committing<'_, impl IdeaCatalog>) {
    // given: an idea in one project and one in another
    let mine = IdeaId::generate(clock::at(1_000));
    let theirs = IdeaId::generate(clock::at(2_000));
    catalog
        .remember(&a_summary(mine, "project_mine", "Mine"))
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&a_summary(theirs, "project_theirs", "Theirs"))
        .await
        .expect("remembering should succeed");

    // when: a batch of the first project is taken
    let batch = catalog
        .in_project_after(&ProjectLink::from("project_mine"), None, 8)
        .await
        .expect("listing should succeed");

    // then: it holds only that project's idea
    assert_eq!(
        batch.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        vec![mine],
        "a sweep of one deleted project must never reach into another"
    );
}

async fn listed(catalog: &Committing<'_, impl IdeaCatalog>) -> Vec<IdeaSummary> {
    catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed")
}

pub async fn a_forgotten_idea_heard_of_late_is_not_remembered_again(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a forgotten idea
    let gone = a_summary(IdeaId::generate(clock::at(1_000)), "project_1", "The Loom");
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    catalog
        .forget(&gone.id, clock::at(5_000))
        .await
        .expect("forgetting should succeed");

    // when: the same summary of it is remembered again
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    // and: a newer one
    let mut newer = gone.clone();
    newer.version = Version::of(newer.version.count() + 1);
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    // then: it stays forgotten
    assert!(
        listed(catalog).await.is_empty(),
        "what was forgotten stays forgotten, however late or new a summary of it comes"
    );
}

pub async fn idea_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: three ideas forgotten at different times
    let (older, newer, latest) = (
        a_summary(IdeaId::generate(clock::at(1_000)), "project_1", "The Loom"),
        a_summary(IdeaId::generate(clock::at(2_000)), "project_1", "The Loom"),
        a_summary(IdeaId::generate(clock::at(3_000)), "project_1", "The Loom"),
    );
    for (summary, laid) in [(&newer, 6_000), (&older, 5_000), (&latest, 9_000)] {
        catalog
            .forget(&summary.id, clock::at(laid))
            .await
            .expect("forgetting should succeed");
    }

    // when: one tombstone laid before a moment is forgotten
    let forgotten = catalog
        .forget_tombstones_laid_before(clock::at(8_000), 1)
        .await
        .expect("forgetting tombstones should succeed");
    // and: all three are remembered again
    for summary in [&older, &newer, &latest] {
        catalog
            .remember(summary)
            .await
            .expect("remembering should succeed");
    }

    // then: one tombstone is gone
    assert_eq!(forgotten, 1);
    // and: only the oldest idea is remembered
    assert_eq!(
        listed(catalog).await,
        vec![older],
        "only the oldest tombstone is gone, so only it can be remembered again"
    );
}

pub async fn an_older_idea_summary_heard_late_changes_nothing(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a remembered idea at its later version
    let summary = a_summary(IdeaId::generate(clock::at(1_000)), "project_1", "The Loom");
    let mut newer = summary.clone();
    newer.version = Version::of(2);
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    // when: an older summary of it is remembered
    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    // then: the later summary stands
    assert_eq!(listed(catalog).await, vec![newer]);
}

pub async fn an_idea_remembered_in_a_transaction_that_is_dropped_was_never_remembered(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: an idea remembered in a transaction
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .remember(
            &mut transaction,
            &a_summary(IdeaId::generate(clock::at(1_000)), "project_1", "The Loom"),
        )
        .await
        .expect("remembering should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: nothing is listed
    assert!(listed(catalog).await.is_empty());
}

pub async fn an_idea_forgotten_in_a_transaction_that_is_dropped_is_still_there_and_can_be_remembered(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a remembered idea
    let mut kept = a_summary(IdeaId::generate(clock::at(1_000)), "project_1", "The Loom");
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");
    // and: a transaction that forgot it
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .forget(&mut transaction, &kept.id, clock::at(5_000))
        .await
        .expect("forgetting should succeed");

    // when: the transaction is dropped
    drop(transaction);
    // and: a newer summary of the idea is remembered
    kept.version = Version::of(kept.version.count() + 1);
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");

    // then: the newer summary is listed
    assert_eq!(
        listed(catalog).await,
        vec![kept],
        "neither the delete nor the tombstone outlives the transaction"
    );
}

pub async fn a_project_has_a_tombstone_from_its_deletion_until_it_is_forgotten(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a project nobody deleted, without a tombstone
    assert!(!catalog.has_project_tombstone("project_1").await);

    // when: its deletion is heard twice
    for seconds in [1_000, 5_000] {
        catalog
            .lay_project_tombstone("project_1", clock::at(seconds))
            .await
            .expect("laying a tombstone should succeed");
    }

    // then: it has a tombstone
    assert!(catalog.has_project_tombstone("project_1").await);
    // and: another project has none
    assert!(!catalog.has_project_tombstone("project_2").await);
    // when: tombstones laid before its second deletion are forgotten
    assert_eq!(
        catalog
            .forget_tombstones_laid_before(clock::at(2_000), u64::MAX)
            .await
            .expect("forgetting should succeed"),
        1,
        "a deletion heard again is the same deletion"
    );
    // then: its tombstone is gone
    assert!(!catalog.has_project_tombstone("project_1").await);
}

pub async fn project_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: three projects deleted one after another
    for (project, seconds) in [
        ("project_newest", 1_002),
        ("project_oldest", 1_000),
        ("project_older", 1_001),
    ] {
        catalog
            .lay_project_tombstone(project, clock::at(seconds))
            .await
            .expect("laying a tombstone should succeed");
    }

    // when: at most two tombstones are forgotten
    assert_eq!(
        catalog
            .forget_tombstones_laid_before(clock::at(9_000), 2)
            .await
            .expect("forgetting should succeed"),
        2
    );
    // then: the two oldest are gone
    assert!(!catalog.has_project_tombstone("project_oldest").await);
    assert!(!catalog.has_project_tombstone("project_older").await);
    // and: the newest stands
    assert!(catalog.has_project_tombstone("project_newest").await);
}

pub async fn a_project_tombstone_laid_in_a_transaction_that_is_dropped_was_never_laid(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a project tombstone laid in a transaction
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .lay_project_tombstone(
            &mut transaction,
            &ProjectLink::from("project_1"),
            clock::at(1_000),
        )
        .await
        .expect("laying a tombstone should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the project has no tombstone
    assert!(!catalog.has_project_tombstone("project_1").await);
}

pub async fn idea_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand(
    catalog: &Committing<'_, impl IdeaCatalog>,
) {
    // given: a forgotten idea
    let gone = a_summary(IdeaId::generate(clock::at(1_000)), "project_1", "The Loom");
    catalog
        .forget(&gone.id, clock::at(5_000))
        .await
        .expect("forgetting should succeed");
    // and: a transaction that forgot its tombstone
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .forget_tombstones_laid_before(&mut transaction, clock::at(8_000), 10)
        .await
        .expect("forgetting tombstones should succeed");

    // when: the transaction is dropped
    drop(transaction);
    // and: the idea is remembered again
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");

    // then: it stays forgotten
    assert!(listed(catalog).await.is_empty());
}

#[macro_export]
macro_rules! catalog_conformance_case {
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
        $crate::catalog_conformance_case!(
            $workbench,
            an_idea_remembered_in_a_transaction_that_is_dropped_was_never_remembered
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_idea_forgotten_in_a_transaction_that_is_dropped_is_still_there_and_can_be_remembered
        );
        $crate::catalog_conformance_case!(
            $workbench,
            idea_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_has_a_tombstone_from_its_deletion_until_it_is_forgotten
        );
        $crate::catalog_conformance_case!(
            $workbench,
            project_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_tombstone_laid_in_a_transaction_that_is_dropped_was_never_laid
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_idea_heard_of_late_is_not_remembered_again
        );
        $crate::catalog_conformance_case!(
            $workbench,
            idea_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_idea_summary_heard_late_changes_nothing
        );
        $crate::catalog_conformance_case!($workbench, a_batch_starts_at_the_oldest_idea);
        $crate::catalog_conformance_case!($workbench, a_batch_after_a_cursor_takes_what_comes_next);
        $crate::catalog_conformance_case!($workbench, a_batch_past_the_end_is_empty);
        $crate::catalog_conformance_case!($workbench, a_batch_stays_inside_its_project);
        $crate::catalog_conformance_case!($workbench, a_remembered_idea_is_listed_in_its_project);
        $crate::catalog_conformance_case!($workbench, a_project_nobody_wrote_in_lists_nothing);
        $crate::catalog_conformance_case!($workbench, ideas_of_other_projects_are_not_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            remembering_the_same_idea_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!($workbench, a_forgotten_idea_is_no_longer_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            forgetting_something_never_remembered_is_harmless
        );
        $crate::catalog_conformance_case!($workbench, the_newest_idea_is_listed_first);
    };
}
