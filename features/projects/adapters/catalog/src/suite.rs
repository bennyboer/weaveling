use std::ops::Deref;

use eventsourcing::Version;
use projects_core::{CatalogError, ProjectCatalog, ProjectId, ProjectName, ProjectSummary};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions, Tried};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: ProjectCatalog;

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

impl<C: ProjectCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub async fn remember(&self, summary: &ProjectSummary) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, summary)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget(&self, id: &ProjectId, at: OffsetDateTime) -> Result<(), CatalogError> {
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

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: ProjectId, name: &str) -> ProjectSummary {
    ProjectSummary {
        id,
        version: Version::of(1),
        name: ProjectName::new(name).expect("a plain name is fine"),
        created_at: at(1_000),
        updated_at: at(1_000),
        trashed_until: None,
    }
}

fn in_the_trash_until(summary: ProjectSummary, until: OffsetDateTime) -> ProjectSummary {
    ProjectSummary {
        version: summary.version.next(),
        trashed_until: Some(until),
        ..summary
    }
}

pub async fn a_project_in_the_trash_is_listed_there_and_nowhere_else(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: two remembered projects
    let kept = a_summary(ProjectId::generate(at(1_000)), "Kept");
    let trashed = a_summary(ProjectId::generate(at(2_000)), "Trashed");
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&trashed)
        .await
        .expect("remembering should succeed");

    // when: one of them is remembered in the trash
    let trashed = in_the_trash_until(trashed, at(9_000));
    catalog
        .remember(&trashed)
        .await
        .expect("remembering should succeed");

    // then: only the other is listed
    assert_eq!(
        catalog.all().await.expect("listing should succeed"),
        vec![kept.clone()]
    );
    // and: only the other is listed among both
    assert_eq!(
        catalog
            .among(&[kept.id, trashed.id])
            .await
            .expect("listing should succeed"),
        vec![kept]
    );
    // and: the trash lists the trashed one
    assert_eq!(
        catalog
            .in_the_trash()
            .await
            .expect("listing the trash should succeed"),
        vec![trashed]
    );
}

pub async fn the_trash_lists_what_is_purged_soonest_first(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a project in the trash until later
    let later = in_the_trash_until(
        a_summary(ProjectId::generate(at(1_000)), "Later"),
        at(9_000),
    );
    // and: a newer one in the trash until sooner
    let sooner = in_the_trash_until(
        a_summary(ProjectId::generate(at(2_000)), "Sooner"),
        at(8_000),
    );

    // when: both are remembered
    catalog
        .remember(&later)
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&sooner)
        .await
        .expect("remembering should succeed");

    // then: the trash lists the sooner one first
    assert_eq!(
        catalog
            .in_the_trash()
            .await
            .expect("listing the trash should succeed"),
        vec![sooner, later]
    );
}

pub async fn a_project_restored_is_listed_again_and_not_in_the_trash(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a project remembered in the trash
    let summary = a_summary(ProjectId::generate(at(1_000)), "Restored");
    let trashed = in_the_trash_until(summary, at(9_000));
    catalog
        .remember(&trashed)
        .await
        .expect("remembering should succeed");

    // when: it is remembered restored
    let restored = ProjectSummary {
        version: trashed.version.next(),
        trashed_until: None,
        ..trashed
    };
    catalog
        .remember(&restored)
        .await
        .expect("remembering should succeed");

    // then: it is listed
    assert_eq!(
        catalog.all().await.expect("listing should succeed"),
        vec![restored]
    );
    // and: the trash is empty
    assert!(
        catalog
            .in_the_trash()
            .await
            .expect("listing the trash should succeed")
            .is_empty()
    );
}

pub async fn a_remembered_project_is_listed(catalog: &Committing<'_, impl ProjectCatalog>) {
    // given: a project
    let summary = a_summary(ProjectId::generate(at(1_000)), "The Weaver's Apprentice");

    // when: it is remembered
    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    // and: the catalog is listed
    let found = catalog.all().await.expect("listing should succeed");

    // then: it is listed
    assert_eq!(found, vec![summary]);
}

pub async fn an_empty_catalog_lists_nothing(catalog: &Committing<'_, impl ProjectCatalog>) {
    // when: an empty catalog is listed
    let found = catalog
        .all()
        .await
        .expect("listing an empty catalog should not fail");

    // then: nothing is listed
    assert!(found.is_empty());
}

pub async fn remembering_the_same_project_again_replaces_what_was_there(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a remembered project
    let id = ProjectId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "Working Title"))
        .await
        .expect("remembering should succeed");

    // when: a later version of it is remembered
    let mut later = a_summary(id, "The Weaver's Apprentice");
    later.version = Version::of(2);
    later.updated_at = at(2_000);
    catalog
        .remember(&later)
        .await
        .expect("remembering should succeed");

    // and: the catalog is listed
    let found = catalog.all().await.expect("listing should succeed");

    // then: only the later version is listed
    assert_eq!(found, vec![later], "a catalog holds one row per project");
}

pub async fn a_forgotten_project_is_no_longer_listed(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a remembered project
    let id = ProjectId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "Abandoned"))
        .await
        .expect("remembering should succeed");

    // when: it is forgotten
    catalog
        .forget(&id, at(5_000))
        .await
        .expect("forgetting should succeed");

    // then: nothing is listed
    assert!(
        catalog
            .all()
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

pub async fn forgetting_something_never_remembered_is_harmless(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // expect: forgetting a project never remembered succeeds
    catalog
        .forget(&ProjectId::generate(at(1_000)), at(5_000))
        .await
        .expect("forgetting an unknown project should not fail");
}

pub async fn the_newest_project_is_listed_first(catalog: &Committing<'_, impl ProjectCatalog>) {
    // given: projects started one after another
    let started: Vec<ProjectId> = (1..=6)
        .map(|nth| ProjectId::generate(at(nth * 1_000)))
        .collect();

    // when: they are remembered oldest first
    for id in &started {
        catalog
            .remember(&a_summary(*id, "A project"))
            .await
            .expect("remembering should succeed");
    }

    // and: the catalog is listed
    let found = catalog.all().await.expect("listing should succeed");

    // then: they are listed newest first
    assert_eq!(
        found.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        started.into_iter().rev().collect::<Vec<_>>(),
        "the project an author started most recently should be the first they see"
    );
}

pub async fn the_moments_a_project_carries_survive_the_round_trip(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a project created and updated at real moments
    let mut summary = a_summary(ProjectId::generate(at(1_000)), "Timestamped");
    summary.created_at = at(1_700_000_000);
    summary.updated_at = at(1_700_086_400);

    // when: it is remembered
    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    // and: the catalog is listed
    let found = catalog.all().await.expect("listing should succeed");

    // then: it is listed with the same moments
    assert_eq!(
        found
            .first()
            .map(|listed| (listed.created_at, listed.updated_at)),
        Some((at(1_700_000_000), at(1_700_086_400))),
        "the listing shows an author when they last touched a project, so the moments must not \
         be rounded or dropped on the way through"
    );
}

pub async fn among_some_ids_only_those_projects_are_listed_newest_first(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: remembered projects started one after another
    let started: Vec<ProjectId> = (1..=4)
        .map(|nth| ProjectId::generate(at(nth * 1_000)))
        .collect();
    for id in &started {
        catalog
            .remember(&a_summary(*id, "A project"))
            .await
            .expect("remembering should succeed");
    }
    // and: a project never remembered
    let never_remembered = ProjectId::generate(at(9_000));

    // when: some of them are listed, one twice, along with the unknown one
    let found = catalog
        .among(&[started[0], started[2], never_remembered, started[0]])
        .await
        .expect("listing should succeed");

    // then: only the remembered ones asked for are listed, newest first, each once
    assert_eq!(
        found.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        vec![started[2], started[0]],
        "an author sees the projects they are in, newest first, each once, and none that is gone"
    );
}

pub async fn among_no_ids_nothing_is_listed(catalog: &Committing<'_, impl ProjectCatalog>) {
    // given: a remembered project
    catalog
        .remember(&a_summary(ProjectId::generate(at(1_000)), "A project"))
        .await
        .expect("remembering should succeed");

    // expect: listing among no ids lists nothing
    assert!(
        catalog
            .among(&[])
            .await
            .expect("listing should succeed")
            .is_empty(),
        "an account in no project sees none"
    );
}

async fn listed(catalog: &Committing<'_, impl ProjectCatalog>) -> Vec<ProjectSummary> {
    catalog.all().await.expect("listing should succeed")
}

pub async fn a_forgotten_project_heard_of_late_is_not_remembered_again(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a forgotten project
    let gone = a_summary(ProjectId::generate(at(1_000)), "Working Title");
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    catalog
        .forget(&gone.id, at(5_000))
        .await
        .expect("forgetting should succeed");

    // when: it is remembered again
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    // and: a newer version of it is remembered
    let mut newer = gone.clone();
    newer.version = Version::of(newer.version.count() + 1);
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    // then: nothing is listed
    assert!(
        listed(catalog).await.is_empty(),
        "what was forgotten stays forgotten, however late or new a summary of it comes"
    );
}

pub async fn project_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: three projects forgotten at different moments
    let (older, newer, latest) = (
        a_summary(ProjectId::generate(at(1_000)), "Working Title"),
        a_summary(ProjectId::generate(at(2_000)), "Working Title"),
        a_summary(ProjectId::generate(at(3_000)), "Working Title"),
    );
    for (summary, laid) in [(&newer, 6_000), (&older, 5_000), (&latest, 9_000)] {
        catalog
            .forget(&summary.id, at(laid))
            .await
            .expect("forgetting should succeed");
    }

    // when: at most one tombstone laid before a moment two of them precede is forgotten
    let forgotten = catalog
        .forget_tombstones_laid_before(at(8_000), 1)
        .await
        .expect("forgetting tombstones should succeed");
    // and: all three are remembered again
    for summary in [&older, &newer, &latest] {
        catalog
            .remember(summary)
            .await
            .expect("remembering should succeed");
    }

    // then: one tombstone was forgotten
    assert_eq!(forgotten, 1);
    // and: only the project forgotten first is listed
    assert_eq!(
        listed(catalog).await,
        vec![older],
        "only the oldest tombstone is gone, so only it can be remembered again"
    );
}

pub async fn an_older_project_summary_heard_late_changes_nothing(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a project remembered at its second version
    let summary = a_summary(ProjectId::generate(at(1_000)), "Working Title");
    let mut newer = summary.clone();
    newer.version = Version::of(2);
    catalog
        .remember(&newer)
        .await
        .expect("remembering should succeed");

    // when: its first version is remembered
    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    // then: the second version is still listed
    assert_eq!(listed(catalog).await, vec![newer]);
}

pub async fn a_project_remembered_in_a_transaction_that_is_dropped_was_never_remembered(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a project remembered in a transaction
    let mut transaction = catalog.begun().await;
    catalog
        .catalog
        .remember(
            &mut transaction,
            &a_summary(ProjectId::generate(at(1_000)), "Working Title"),
        )
        .await
        .expect("remembering should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: nothing is listed
    assert!(listed(catalog).await.is_empty());
}

pub async fn a_project_forgotten_in_a_transaction_that_is_dropped_is_still_there_and_can_be_remembered(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a remembered project
    let mut kept = a_summary(ProjectId::generate(at(1_000)), "Working Title");
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");
    // and: it forgotten in a transaction
    let mut transaction = catalog.begun().await;
    catalog
        .catalog
        .forget(&mut transaction, &kept.id, at(5_000))
        .await
        .expect("forgetting should succeed");

    // when: the transaction is dropped
    drop(transaction);
    // and: a newer version of the project is remembered
    kept.version = Version::of(2);
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");

    // then: the newer version is listed
    assert_eq!(
        listed(catalog).await,
        vec![kept],
        "neither the delete nor the tombstone outlives the transaction"
    );
}

pub async fn tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand(
    catalog: &Committing<'_, impl ProjectCatalog>,
) {
    // given: a forgotten project
    let gone = a_summary(ProjectId::generate(at(1_000)), "Working Title");
    catalog
        .forget(&gone.id, at(5_000))
        .await
        .expect("forgetting should succeed");
    // and: its tombstone forgotten in a transaction
    let mut transaction = catalog.begun().await;
    catalog
        .catalog
        .forget_tombstones_laid_before(&mut transaction, at(8_000), 10)
        .await
        .expect("forgetting tombstones should succeed");

    // when: the transaction is dropped
    drop(transaction);
    // and: the project is remembered again
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");

    // then: nothing is listed
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
        $crate::catalog_conformance_case!($workbench, a_project_remembered_in_a_transaction_that_is_dropped_was_never_remembered);
        $crate::catalog_conformance_case!($workbench, a_project_forgotten_in_a_transaction_that_is_dropped_is_still_there_and_can_be_remembered);
        $crate::catalog_conformance_case!($workbench, tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand);
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_project_heard_of_late_is_not_remembered_again
        );
        $crate::catalog_conformance_case!(
            $workbench,
            project_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_project_summary_heard_late_changes_nothing
        );
        $crate::catalog_conformance_case!($workbench, a_remembered_project_is_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_in_the_trash_is_listed_there_and_nowhere_else
        );
        $crate::catalog_conformance_case!($workbench, the_trash_lists_what_is_purged_soonest_first);
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_restored_is_listed_again_and_not_in_the_trash
        );
        $crate::catalog_conformance_case!($workbench, an_empty_catalog_lists_nothing);
        $crate::catalog_conformance_case!(
            $workbench,
            remembering_the_same_project_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!($workbench, a_forgotten_project_is_no_longer_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            forgetting_something_never_remembered_is_harmless
        );
        $crate::catalog_conformance_case!($workbench, the_newest_project_is_listed_first);
        $crate::catalog_conformance_case!(
            $workbench,
            among_some_ids_only_those_projects_are_listed_newest_first
        );
        $crate::catalog_conformance_case!($workbench, among_no_ids_nothing_is_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            the_moments_a_project_carries_survive_the_round_trip
        );
    };
}
