use eventsourcing::{AccountId, Agent};
use readings_core::{
    CatalogError, Extent, ProjectLink, ReadingCopyCatalog, ReadingCopyId, ReadingCopyName,
    ReadingCopySummary, Times,
};
use time::{Duration, OffsetDateTime};
use transacting::Tried;

use crate::suite::Committing;

impl<C: ReadingCopyCatalog> Committing<'_, C> {
    pub async fn remember(&self, summary: &ReadingCopySummary) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, summary)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }

    pub async fn forget(&self, copy: &ReadingCopyId) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, copy)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }
}

fn at(seconds: i64) -> OffsetDateTime {
    clock::at(seconds)
}

fn a_copy(project: &str, named: &str, made_at: OffsetDateTime) -> ReadingCopySummary {
    ReadingCopySummary {
        copy: ReadingCopyId::generate(made_at),
        project: ProjectLink::from(project),
        name: ReadingCopyName::new(named).expect("a plain name is fine"),
        made_by: Agent::Account(AccountId::from("account_hedy")),
        made_at,
        extent: Extent::Book,
        times: Times {
            open_until: Some(made_at + Duration::days(30)),
            deleted_on: Some(made_at + Duration::days(90)),
        },
    }
}

async fn remembered(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
    summary: &ReadingCopySummary,
) {
    catalog
        .remember(summary)
        .await
        .expect("remembering should succeed");
}

async fn copies_of(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
    project: &str,
) -> Vec<ReadingCopySummary> {
    catalog
        .copies_of(&ProjectLink::from(project))
        .await
        .expect("asking should succeed")
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: a copy kept
    let kept = a_copy("project_loom", "For Lena", at(10));
    remembered(catalog, &kept).await;

    // when: in a transaction that is dropped, another copy is kept and the first forgotten
    {
        let mut transaction = catalog.begun().await;
        catalog
            .uncommitted()
            .remember(&mut transaction, &a_copy("project_loom", "Gone", at(20)))
            .await
            .expect("remembering should succeed");
        catalog
            .uncommitted()
            .forget(&mut transaction, &kept.copy)
            .await
            .expect("forgetting should succeed");
    }

    // then: only what was there before stands
    assert_eq!(copies_of(catalog, "project_loom").await, vec![kept]);
}

pub async fn a_project_with_no_copies_lists_none(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: a copy of another project
    remembered(catalog, &a_copy("project_other", "For Lena", at(10))).await;

    // expect: none for a project nobody made a copy of
    assert_eq!(copies_of(catalog, "project_loom").await, vec![]);
}

pub async fn a_project_s_copies_are_listed_newest_first(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: three copies of a project, two made at the same moment, and one of another project
    let oldest = a_copy("project_loom", "First draft", at(10));
    let newest = a_copy("project_loom", "Second draft", at(30));
    let beside_newest = a_copy("project_loom", "Second draft, again", at(30));
    for summary in [&newest, &oldest, &beside_newest] {
        remembered(catalog, summary).await;
    }
    remembered(catalog, &a_copy("project_other", "Elsewhere", at(20))).await;

    // when: the project's copies are asked for
    let listed = copies_of(catalog, "project_loom").await;

    // then: they are its own, the newest first, those made together by their ids, the later first
    let (earlier, later) = if newest.copy < beside_newest.copy {
        (newest, beside_newest)
    } else {
        (beside_newest, newest)
    };
    assert_eq!(listed, vec![later, earlier, oldest]);
}

pub async fn a_copy_reads_back_as_it_was_kept_whoever_made_it_and_whatever_it_holds(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: copies made by every kind of agent, holding every extent, with and without ends
    let kept = [
        ReadingCopySummary {
            made_by: Agent::Local,
            extent: Extent::Sections(3),
            times: Times {
                open_until: None,
                deleted_on: None,
            },
            ..a_copy("project_loom", "Local", at(10))
        },
        ReadingCopySummary {
            made_by: Agent::System,
            extent: Extent::Scenes(2),
            times: Times {
                open_until: Some(at(10) + Duration::milliseconds(1_500)),
                deleted_on: None,
            },
            ..a_copy("project_loom", "System", at(20))
        },
        a_copy("project_loom", "Hedy", at(30)),
    ];
    for summary in &kept {
        remembered(catalog, summary).await;
    }

    // expect: each reads back as it was kept
    let mut newest_first = kept.to_vec();
    newest_first.reverse();
    assert_eq!(copies_of(catalog, "project_loom").await, newest_first);
}

pub async fn a_copy_remembered_again_is_kept_as_it_stands_now(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: a copy kept
    let made = a_copy("project_loom", "For Lena", at(10));
    remembered(catalog, &made).await;

    // when: it is remembered again, renamed and extended
    let changed = ReadingCopySummary {
        name: ReadingCopyName::new("For Lena, again").expect("a plain name is fine"),
        times: Times {
            open_until: None,
            deleted_on: None,
        },
        ..made
    };
    remembered(catalog, &changed).await;

    // then: it is listed once, as it now stands
    assert_eq!(copies_of(catalog, "project_loom").await, vec![changed]);
}

pub async fn a_forgotten_copy_is_no_longer_listed(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: two copies of a project
    let forgotten = a_copy("project_loom", "For Lena", at(10));
    let staying = a_copy("project_loom", "For Ada", at(20));
    remembered(catalog, &forgotten).await;
    remembered(catalog, &staying).await;

    // when: one is forgotten, and a copy never kept as well
    for copy in [forgotten.copy, ReadingCopyId::generate(at(40))] {
        catalog
            .forget(&copy)
            .await
            .expect("forgetting should succeed");
    }

    // then: only the other one is listed
    assert_eq!(copies_of(catalog, "project_loom").await, vec![staying]);
}

pub async fn copies_due_for_deletion_come_earliest_first_and_no_more_than_asked(
    catalog: &Committing<'_, impl ReadingCopyCatalog>,
) {
    // given: copies deleted on different days, one kept for good, and one of another project
    let deleted_on = |copy: ReadingCopySummary, day: Option<i64>| ReadingCopySummary {
        times: Times {
            open_until: None,
            deleted_on: day.map(|day| at(0) + Duration::days(day)),
        },
        ..copy
    };
    let third = deleted_on(a_copy("project_loom", "Third", at(1)), Some(30));
    let first = deleted_on(a_copy("project_loom", "First", at(2)), Some(10));
    let second = deleted_on(a_copy("project_other", "Second", at(3)), Some(20));
    let later = deleted_on(a_copy("project_loom", "Later", at(4)), Some(40));
    let kept = deleted_on(a_copy("project_loom", "Kept", at(5)), None);
    for summary in [&third, &first, &second, &later, &kept] {
        remembered(catalog, summary).await;
    }
    let due = |day: i64, at_most: u64| async move {
        catalog
            .due_for_deletion(at(0) + Duration::days(day), at_most)
            .await
            .expect("asking should succeed")
    };

    // expect: by day 30, those due then or earlier, of every project, the earliest first
    assert_eq!(due(30, 10).await, vec![first.copy, second.copy, third.copy]);
    // and: no more than asked
    assert_eq!(due(30, 2).await, vec![first.copy, second.copy]);
    // and: none before the first is due
    assert_eq!(due(9, 10).await, vec![]);
}

macro_rules! copies_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::copies_suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! copies_conformance_tests {
    ($workbench:ty) => {
        $crate::copies_suite::copies_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::copies_suite::copies_conformance_case!(
            $workbench,
            a_project_with_no_copies_lists_none
        );
        $crate::copies_suite::copies_conformance_case!(
            $workbench,
            a_project_s_copies_are_listed_newest_first
        );
        $crate::copies_suite::copies_conformance_case!(
            $workbench,
            a_copy_reads_back_as_it_was_kept_whoever_made_it_and_whatever_it_holds
        );
        $crate::copies_suite::copies_conformance_case!(
            $workbench,
            a_copy_remembered_again_is_kept_as_it_stands_now
        );
        $crate::copies_suite::copies_conformance_case!(
            $workbench,
            a_forgotten_copy_is_no_longer_listed
        );
        $crate::copies_suite::copies_conformance_case!(
            $workbench,
            copies_due_for_deletion_come_earliest_first_and_no_more_than_asked
        );
    };
}

pub(crate) use {copies_conformance_case, copies_conformance_tests};
