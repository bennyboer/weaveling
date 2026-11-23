use std::collections::HashMap;

use feedback_core::{
    CatalogError, KnownReadingCopy, ProjectLink, ReadingCopyCatalog, ReadingCopyLink,
};
use time::OffsetDateTime;
use transacting::Tried;

use crate::suite::{Committing, unreachable};

impl<C: ReadingCopyCatalog> Committing<'_, C> {
    pub async fn keep(
        &self,
        copy: &ReadingCopyLink,
        known: &KnownReadingCopy,
        version: u64,
    ) -> Result<bool, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .catalog
                .keep(transaction, copy, known, version)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget(
        &self,
        copy: &ReadingCopyLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, copy, at)
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

fn at(seconds: i64) -> OffsetDateTime {
    clock::at(seconds)
}

fn a_copy(named: &str) -> ReadingCopyLink {
    ReadingCopyLink::from(named)
}

/// A copy of the loom project, open until a time, or without end.
fn open_until(until: Option<i64>) -> KnownReadingCopy {
    KnownReadingCopy {
        project: ProjectLink::from("project_loom"),
        open_until: until.map(at),
    }
}

async fn kept(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
    copy: &str,
    known: KnownReadingCopy,
    version: u64,
) -> bool {
    catalog
        .keep(&a_copy(copy), &known, version)
        .await
        .expect("keeping should succeed")
}

async fn known(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
    copies: &[&str],
) -> HashMap<ReadingCopyLink, KnownReadingCopy> {
    let asked: Vec<ReadingCopyLink> = copies.iter().map(|copy| a_copy(copy)).collect();

    catalog
        .uncommitted()
        .known(&asked)
        .await
        .expect("asking should succeed")
}

fn copies(pairs: &[(&str, KnownReadingCopy)]) -> HashMap<ReadingCopyLink, KnownReadingCopy> {
    pairs
        .iter()
        .map(|(copy, known)| (a_copy(copy), known.clone()))
        .collect()
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: a copy kept
    kept(catalog, "copy_loom", open_until(Some(100)), 1).await;

    // when: in a transaction that is dropped, another copy is kept and the first forgotten
    {
        let mut transaction = catalog.begun().await;
        catalog
            .uncommitted()
            .keep(&mut transaction, &a_copy("copy_weft"), &open_until(None), 1)
            .await
            .expect("keeping should succeed");
        catalog
            .uncommitted()
            .forget(&mut transaction, &a_copy("copy_loom"), at(10))
            .await
            .expect("forgetting should succeed");
    }

    // then: only what was there before stands
    assert_eq!(
        known(catalog, &["copy_loom", "copy_weft"]).await,
        copies(&[("copy_loom", open_until(Some(100)))])
    );
}

pub async fn the_copies_asked_for_are_those_kept(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: two copies kept, one open without end
    kept(catalog, "copy_loom", open_until(Some(100)), 1).await;
    kept(catalog, "copy_weft", open_until(None), 1).await;

    // expect: those asked for that are kept, and none for a copy never heard of
    assert_eq!(
        known(catalog, &["copy_weft", "copy_shuttle"]).await,
        copies(&[("copy_weft", open_until(None))])
    );
}

pub async fn a_newer_version_replaces_what_was_kept_and_an_older_one_does_not(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: a copy kept as made
    assert!(kept(catalog, "copy_loom", open_until(Some(100)), 1).await);

    // when: it is extended
    let extended = kept(catalog, "copy_loom", open_until(Some(200)), 2).await;
    // then: it is kept so
    assert!(extended);
    assert_eq!(
        known(catalog, &["copy_loom"]).await,
        copies(&[("copy_loom", open_until(Some(200)))])
    );

    // when: the message it was made by comes late, or the same one again
    let late = kept(catalog, "copy_loom", open_until(Some(100)), 1).await;
    let again = kept(catalog, "copy_loom", open_until(Some(100)), 2).await;
    // then: neither is kept, and the newer stands
    assert!(!late);
    assert!(!again);
    assert_eq!(
        known(catalog, &["copy_loom"]).await,
        copies(&[("copy_loom", open_until(Some(200)))])
    );
}

pub async fn a_deleted_copy_is_forgotten_and_a_late_message_does_not_bring_it_back(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: a copy kept, then deleted
    kept(catalog, "copy_loom", open_until(Some(100)), 1).await;
    catalog
        .forget(&a_copy("copy_loom"), at(50))
        .await
        .expect("forgetting should succeed");

    // then: it is not known
    assert_eq!(known(catalog, &["copy_loom"]).await, copies(&[]));

    // when: a message about it comes late
    let late = kept(catalog, "copy_loom", open_until(Some(200)), 2).await;
    // then: it is not kept, and stays unknown
    assert!(!late);
    assert_eq!(known(catalog, &["copy_loom"]).await, copies(&[]));
}

pub async fn old_tombstones_are_forgotten_oldest_first_and_no_more_than_asked(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: three copies deleted at different times
    for (copy, laid) in [("copy_late", 30), ("copy_early", 10), ("copy_middle", 20)] {
        catalog
            .forget(&a_copy(copy), at(laid))
            .await
            .expect("forgetting should succeed");
    }

    // when: at most one tombstone laid before 25 is forgotten, then the rest laid before it
    let first = catalog
        .forget_tombstones_laid_before(at(25), 1)
        .await
        .expect("forgetting tombstones should succeed");
    let then = catalog
        .forget_tombstones_laid_before(at(25), 10)
        .await
        .expect("forgetting tombstones should succeed");

    // then: one was forgotten, then the other laid before 25, and none laid after it
    assert_eq!((first, then), (1, 1));
    // and: the earliest copy, its tombstone forgotten first, may be kept again
    assert!(kept(catalog, "copy_early", open_until(None), 1).await);
    // and: the latest copy's tombstone still keeps it away
    assert!(!kept(catalog, "copy_late", open_until(None), 1).await);
}

impl<C: ReadingCopyCatalog> Committing<'_, C> {
    pub async fn drafts_dropped(&self, copy: &ReadingCopyLink) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .drafts_dropped(transaction, copy)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }
}

async fn closed_with_drafts_left(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
    by: i64,
    at_most: usize,
) -> Vec<ReadingCopyLink> {
    catalog
        .uncommitted()
        .closed_with_drafts_left(at(by), at_most)
        .await
        .expect("asking should succeed")
}

pub async fn the_copies_closed_by_a_time_are_found_until_their_drafts_are_dropped(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: copies closing at 20 and 10, one closing at 50, and one open without end
    kept(catalog, "copy_late", open_until(Some(20)), 1).await;
    kept(catalog, "copy_early", open_until(Some(10)), 1).await;
    kept(catalog, "copy_open", open_until(Some(50)), 1).await;
    kept(catalog, "copy_endless", open_until(None), 1).await;

    // expect: by 30, the two closed, the earliest first, and no more than asked
    assert_eq!(
        closed_with_drafts_left(catalog, 30, 10).await,
        [a_copy("copy_early"), a_copy("copy_late")]
    );
    assert_eq!(
        closed_with_drafts_left(catalog, 30, 1).await,
        [a_copy("copy_early")]
    );

    // when: the earliest copy's drafts are dropped
    catalog
        .drafts_dropped(&a_copy("copy_early"))
        .await
        .expect("keeping that drafts were dropped should succeed");
    // then: it is found no more
    assert_eq!(
        closed_with_drafts_left(catalog, 30, 10).await,
        [a_copy("copy_late")]
    );

    // when: it is extended to close again at 40
    kept(catalog, "copy_early", open_until(Some(40)), 2).await;
    // then: it is found again once closed again, its new drafts to be dropped too
    assert_eq!(
        closed_with_drafts_left(catalog, 30, 10).await,
        [a_copy("copy_late")]
    );
    assert_eq!(
        closed_with_drafts_left(catalog, 45, 10).await,
        [a_copy("copy_late"), a_copy("copy_early")]
    );
}

macro_rules! reading_copies_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::reading_copies_suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! reading_copies_conformance_tests {
    ($workbench:ty) => {
        $crate::reading_copies_suite::reading_copies_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::reading_copies_suite::reading_copies_conformance_case!(
            $workbench,
            the_copies_asked_for_are_those_kept
        );
        $crate::reading_copies_suite::reading_copies_conformance_case!(
            $workbench,
            a_newer_version_replaces_what_was_kept_and_an_older_one_does_not
        );
        $crate::reading_copies_suite::reading_copies_conformance_case!(
            $workbench,
            a_deleted_copy_is_forgotten_and_a_late_message_does_not_bring_it_back
        );
        $crate::reading_copies_suite::reading_copies_conformance_case!(
            $workbench,
            old_tombstones_are_forgotten_oldest_first_and_no_more_than_asked
        );
        $crate::reading_copies_suite::reading_copies_conformance_case!(
            $workbench,
            the_copies_closed_by_a_time_are_found_until_their_drafts_are_dropped
        );
    };
}

pub(crate) use {reading_copies_conformance_case, reading_copies_conformance_tests};
