use std::ops::Deref;

use std::slice;
use transacting::{Transaction, Transactions, Tried};

use eventsourcing::{AccountId, Version};
use notifications_core::{
    About, CatalogError, NoticeCatalog, NoticeId, NoticeSummary, Person, ProjectLink, ProjectName,
    Role,
};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: NoticeCatalog;

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

impl<C: NoticeCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn remember(&self, summary: &NoticeSummary) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, summary)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_person(transaction, person)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn remember_project_name(&self, named: &ProjectName) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember_project_name(transaction, named)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .lay_project_tombstone(transaction, project, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget(&self, notice: &NoticeId) -> Result<(), CatalogError> {
        self.forget_at(notice, OffsetDateTime::UNIX_EPOCH).await
    }

    pub async fn forget_at(
        &self,
        notice: &NoticeId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, notice, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_notice_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .catalog
                .forget_notice_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_project_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .catalog
                .forget_project_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
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

pub fn a_summary(account: &str, noted_at: i64, version: u64, dismissed: bool) -> NoticeSummary {
    NoticeSummary {
        id: NoticeId::generate(at(noted_at)),
        version: Version::of(version),
        account: AccountId::from(account),
        about: About::RoleChanged {
            project: ProjectLink::from("project_1"),
            role: Role::Author,
            by: Some(AccountId::from("account_ada")),
        },
        noted_at: at(noted_at),
        dismissed_at: dismissed.then(|| at(noted_at + 600)),
    }
}

async fn open_for(
    catalog: &Committing<'_, impl NoticeCatalog>,
    account: &str,
) -> Vec<NoticeSummary> {
    catalog
        .open_for(&AccountId::from(account))
        .await
        .expect("listing should succeed")
}

async fn remembered(catalog: &Committing<'_, impl NoticeCatalog>, summary: &NoticeSummary) {
    catalog
        .remember(summary)
        .await
        .expect("remembering should succeed");
}

pub async fn an_account_s_open_notices_are_listed_for_it_alone(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: an open notice for bo and one for cy
    let bo_s = a_summary("account_bo", 1_000, 1, false);
    remembered(catalog, &bo_s).await;
    remembered(catalog, &a_summary("account_cy", 1_000, 1, false)).await;

    // then: bo's alone are listed for bo
    assert_eq!(open_for(catalog, "account_bo").await, vec![bo_s]);
    // and: none for an account without any
    assert!(open_for(catalog, "account_dee").await.is_empty());
}

pub async fn the_newest_notice_comes_first(catalog: &Committing<'_, impl NoticeCatalog>) {
    // given: an older and a newer notice for bo
    let older = a_summary("account_bo", 1_000, 1, false);
    let newer = a_summary("account_bo", 2_000, 1, false);
    remembered(catalog, &older).await;
    remembered(catalog, &newer).await;

    // then: the newer is listed first
    assert_eq!(open_for(catalog, "account_bo").await, vec![newer, older]);
}

pub async fn a_dismissed_notice_is_listed_no_more(catalog: &Committing<'_, impl NoticeCatalog>) {
    // given: a listed notice
    let noted = a_summary("account_bo", 1_000, 1, false);
    remembered(catalog, &noted).await;

    // when: its dismissing is remembered
    remembered(
        catalog,
        &NoticeSummary {
            version: Version::of(2),
            dismissed_at: Some(at(1_600)),
            ..noted
        },
    )
    .await;

    // then: it is listed no more
    assert!(open_for(catalog, "account_bo").await.is_empty());
}

pub async fn a_version_heard_late_changes_nothing(catalog: &Committing<'_, impl NoticeCatalog>) {
    // given: a dismissed notice
    let noted = a_summary("account_bo", 1_000, 1, false);
    let dismissed = NoticeSummary {
        version: Version::of(2),
        dismissed_at: Some(at(1_600)),
        ..noted.clone()
    };
    remembered(catalog, &dismissed).await;

    // when: its noting is heard late
    remembered(catalog, &noted).await;

    // then: it stays dismissed
    assert!(
        open_for(catalog, "account_bo").await.is_empty(),
        "a notice dismissed stays dismissed, however late its noting is heard"
    );
}

pub async fn every_role_and_who_changed_it_read_back_as_they_were_kept(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: notices about each role, with and without who changed it
    let mut kept = Vec::new();
    for (at, role, by) in [
        (3_000, Role::Owner, Some("account_ada")),
        (2_000, Role::Author, None),
        (1_000, Role::Reader, Some("account_cy")),
    ] {
        let summary = NoticeSummary {
            about: About::RoleChanged {
                project: ProjectLink::from("project_1"),
                role,
                by: by.map(AccountId::from),
            },
            ..a_summary("account_bo", at, 1, false)
        };
        remembered(catalog, &summary).await;
        kept.push(summary);
    }

    // then: they read back as they were kept
    assert_eq!(open_for(catalog, "account_bo").await, kept);
}

pub async fn a_person_is_named_as_last_told_and_nobody_unknown_is_named(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: a person told twice
    for name in ["Ada", "Ada Lovelace"] {
        catalog
            .remember_person(&Person {
                account: AccountId::from("account_ada"),
                name: name.to_owned(),
            })
            .await
            .expect("remembering should succeed");
    }

    // when: they and someone unknown are asked for
    let named = catalog
        .people(&[
            AccountId::from("account_ada"),
            AccountId::from("account_nobody"),
        ])
        .await
        .expect("asking should succeed");

    // then: the person alone is named, as last told
    assert_eq!(
        named,
        vec![Person {
            account: AccountId::from("account_ada"),
            name: "Ada Lovelace".to_owned(),
        }]
    );
}

fn a_name(version: u64, name: &str) -> ProjectName {
    ProjectName {
        project: ProjectLink::from("project_1"),
        version: Version::of(version),
        name: name.to_owned(),
    }
}

pub async fn a_project_keeps_its_newest_name_however_late_an_older_one_is_heard(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: a project's names told out of order
    for told in [
        a_name(1, "Shared"),
        a_name(3, "Shared Again"),
        a_name(2, "Between"),
    ] {
        catalog
            .remember_project_name(&told)
            .await
            .expect("remembering should succeed");
    }

    // when: its and an unknown project's names are asked for
    let names = catalog
        .project_names(&[
            ProjectLink::from("project_1"),
            ProjectLink::from("project_unknown"),
        ])
        .await
        .expect("asking should succeed");

    // then: it alone is named, by its newest
    assert_eq!(names, vec![a_name(3, "Shared Again")]);
}

fn in_project(summary: NoticeSummary, project: &str) -> NoticeSummary {
    NoticeSummary {
        about: About::RoleChanged {
            project: ProjectLink::from(project),
            role: Role::Author,
            by: None,
        },
        ..summary
    }
}

pub async fn a_deleted_project_s_notices_are_listed_no_more(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: notices for bo about two projects
    let kept = in_project(a_summary("account_bo", 2_000, 1, false), "project_2");
    remembered(
        catalog,
        &in_project(a_summary("account_bo", 1_000, 1, false), "project_1"),
    )
    .await;
    remembered(catalog, &kept).await;

    // when: one project's tombstone is laid twice
    for _ in 0..2 {
        catalog
            .lay_project_tombstone(&ProjectLink::from("project_1"), at(3_000))
            .await
            .expect("laying a tombstone should succeed however often");
    }

    // then: only the other's notice is listed
    assert_eq!(open_for(catalog, "account_bo").await, vec![kept]);
}

pub async fn a_deleted_project_s_name_is_forgotten_and_one_heard_late_is_not_kept(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: a project's name known
    let project = ProjectLink::from("project_1");
    catalog
        .remember_project_name(&a_name(1, "Shared"))
        .await
        .expect("remembering should succeed");

    // when: the project is deleted
    catalog
        .lay_project_tombstone(&project, at(3_000))
        .await
        .expect("laying a tombstone should succeed");
    // and: a name of it is heard late
    catalog
        .remember_project_name(&a_name(2, "Renamed meanwhile"))
        .await
        .expect("a name heard late is no failure");

    // then: it is named no more
    assert!(
        catalog
            .project_names(slice::from_ref(&project))
            .await
            .expect("asking should succeed")
            .is_empty()
    );
    // and: it has a tombstone
    assert!(
        catalog
            .has_project_tombstone(&project)
            .await
            .expect("asking should succeed")
    );
    // and: another project has none
    assert!(
        !catalog
            .has_project_tombstone(&ProjectLink::from("project_2"))
            .await
            .expect("asking should succeed")
    );
}

pub async fn the_accounts_with_open_notices_about_a_project_are_named_once_each(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: open notices about a project for two accounts, one of them twice
    remembered(
        catalog,
        &in_project(a_summary("account_bo", 1_000, 1, false), "project_1"),
    )
    .await;
    remembered(
        catalog,
        &in_project(a_summary("account_bo", 2_000, 1, false), "project_1"),
    )
    .await;
    remembered(
        catalog,
        &in_project(a_summary("account_cy", 1_000, 1, false), "project_1"),
    )
    .await;
    // and: one dismissed
    remembered(
        catalog,
        &in_project(a_summary("account_dee", 1_000, 2, true), "project_1"),
    )
    .await;
    // and: one about another project
    remembered(
        catalog,
        &in_project(a_summary("account_eve", 1_000, 1, false), "project_2"),
    )
    .await;

    // when: the accounts noticed about the project are asked for
    let noticed = catalog
        .accounts_noticed_about(&ProjectLink::from("project_1"))
        .await
        .expect("asking should succeed");

    // then: those with open notices are named once each
    assert_eq!(
        noticed,
        vec![AccountId::from("account_bo"), AccountId::from("account_cy")],
        "whoever has dismissed theirs has nothing to be told"
    );
}

pub async fn notices_long_dismissed_long_noted_or_of_a_deleted_project_are_due_and_forgotten(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: notices long and lately dismissed, long and lately noted, and one of a deleted project
    let long_dismissed = NoticeSummary {
        version: Version::of(2),
        dismissed_at: Some(at(1_000)),
        ..a_summary("account_bo", 900, 1, false)
    };
    let lately_dismissed = NoticeSummary {
        version: Version::of(2),
        dismissed_at: Some(at(5_000)),
        ..a_summary("account_bo", 900, 1, false)
    };
    let long_noted = a_summary("account_bo", 1_000, 1, false);
    let lately_noted = a_summary("account_bo", 5_000, 1, false);
    let of_a_deleted_project = in_project(a_summary("account_bo", 5_000, 1, false), "project_gone");
    for summary in [
        &long_dismissed,
        &lately_dismissed,
        &long_noted,
        &lately_noted,
        &of_a_deleted_project,
    ] {
        remembered(catalog, summary).await;
    }
    catalog
        .lay_project_tombstone(&ProjectLink::from("project_gone"), at(5_000))
        .await
        .expect("laying a tombstone should succeed");

    // when: what is due is asked for
    let mut due = catalog
        .due_for_forgetting(at(2_000), at(2_000), 10)
        .await
        .expect("asking should succeed");
    // then: the long dismissed, the long noted and the deleted project's are due
    due.sort();
    let mut expected = vec![long_dismissed.id, long_noted.id, of_a_deleted_project.id];
    expected.sort();
    assert_eq!(due, expected);
    // and: no more at once than asked
    assert_eq!(
        catalog
            .due_for_forgetting(at(2_000), at(2_000), 2)
            .await
            .expect("asking should succeed")
            .len(),
        2,
        "no more at once than asked for"
    );

    // when: they are forgotten
    for notice in &due {
        catalog
            .forget(notice)
            .await
            .expect("forgetting should succeed");
    }
    // then: nothing is due
    assert!(
        catalog
            .due_for_forgetting(at(2_000), at(2_000), 10)
            .await
            .expect("asking should succeed")
            .is_empty()
    );
    // and: the lately noted alone is listed
    assert_eq!(open_for(catalog, "account_bo").await, vec![lately_noted]);
}

pub async fn old_project_tombstones_are_forgotten_oldest_first_and_no_more_than_asked(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: project tombstones laid at different moments
    for (project, laid) in [
        ("project_a", 1_000),
        ("project_b", 2_000),
        ("project_c", 9_000),
    ] {
        catalog
            .lay_project_tombstone(&ProjectLink::from(project), at(laid))
            .await
            .expect("laying a tombstone should succeed");
    }

    // when: at most one laid before a moment is forgotten, and then the rest
    let first = catalog
        .forget_project_tombstones_laid_before(at(5_000), 1)
        .await
        .expect("forgetting should succeed");
    let oldest_gone_first = !catalog
        .has_project_tombstone(&ProjectLink::from("project_a"))
        .await
        .expect("asking should succeed")
        && catalog
            .has_project_tombstone(&ProjectLink::from("project_b"))
            .await
            .expect("asking should succeed");
    let rest = catalog
        .forget_project_tombstones_laid_before(at(5_000), 10)
        .await
        .expect("forgetting should succeed");

    // then: one is forgotten each time
    assert_eq!((first, rest), (1, 1));
    // and: the oldest first
    assert!(oldest_gone_first, "the oldest goes first");
    // and: the one laid after the moment stands
    for (project, kept) in [
        ("project_a", false),
        ("project_b", false),
        ("project_c", true),
    ] {
        assert_eq!(
            catalog
                .has_project_tombstone(&ProjectLink::from(project))
                .await
                .expect("asking should succeed"),
            kept,
            "{project}"
        );
    }
}

pub async fn a_forgotten_notice_heard_of_late_is_not_remembered_again(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: a forgotten notice
    let notice = a_summary("account_bo", 1_000, 1, false);
    remembered(catalog, &notice).await;
    catalog
        .forget_at(&notice.id, at(2_000))
        .await
        .expect("forgetting should succeed");

    // when: a newer version of it is heard late
    remembered(
        catalog,
        &NoticeSummary {
            version: Version::of(2),
            ..notice.clone()
        },
    )
    .await;

    // then: it is not listed
    assert!(
        open_for(catalog, "account_bo").await.is_empty(),
        "a cataloguing that read the notice before it was forgotten must not bring it back"
    );
}

pub async fn notice_tombstones_are_forgotten_oldest_first_and_no_more_than_asked(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: notices forgotten at different moments
    let notices: Vec<NoticeSummary> = (1..=3)
        .map(|nth| a_summary("account_bo", nth * 1_000, 1, false))
        .collect();
    for (notice, seconds) in notices.iter().zip([3_000, 1_000, 2_000]) {
        catalog
            .forget_at(&notice.id, at(seconds))
            .await
            .expect("forgetting should succeed");
    }

    // then: forgetting at most two of their tombstones forgets two
    assert_eq!(
        catalog
            .forget_notice_tombstones_laid_before(at(9_000), 2)
            .await
            .expect("forgetting tombstones should succeed"),
        2
    );
    // when: each notice is heard of late
    for notice in &notices {
        remembered(catalog, notice).await;
    }
    // then: only those whose tombstones are gone are listed again, the oldest
    let mut open: Vec<NoticeId> = open_for(catalog, "account_bo")
        .await
        .iter()
        .map(|summary| summary.id)
        .collect();
    open.sort();
    let mut forgotten_first = vec![notices[1].id, notices[2].id];
    forgotten_first.sort();
    assert_eq!(open, forgotten_first, "the oldest tombstones went first");
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: a listed notice and a project's name
    let kept = a_summary("account_bo", 1_000, 1, false);
    remembered(catalog, &kept).await;
    let named = ProjectName {
        project: ProjectLink::from("project_1"),
        version: Version::of(1),
        name: "Shared".to_owned(),
    };
    catalog
        .remember_project_name(&named)
        .await
        .expect("remembering should succeed");
    // when: a transaction remembers, names a person, forgets and lays a tombstone
    let mut transaction = catalog.begun().await;
    let uncommitted = catalog.uncommitted();
    uncommitted
        .remember(&mut transaction, &a_summary("account_bo", 2_000, 1, false))
        .await
        .expect("remembering should succeed");
    uncommitted
        .remember_person(
            &mut transaction,
            &Person {
                account: AccountId::from("account_ada"),
                name: "Ada".to_owned(),
            },
        )
        .await
        .expect("remembering should succeed");
    uncommitted
        .forget(&mut transaction, &kept.id, at(3_000))
        .await
        .expect("forgetting should succeed");
    uncommitted
        .lay_project_tombstone(&mut transaction, &ProjectLink::from("project_1"), at(3_000))
        .await
        .expect("laying a tombstone should succeed");

    // and: it is dropped
    drop(transaction);

    // then: the list is as it was
    assert_eq!(open_for(catalog, "account_bo").await, vec![kept]);
    // and: nobody is named
    assert!(
        catalog
            .people(&[AccountId::from("account_ada")])
            .await
            .expect("naming should succeed")
            .is_empty()
    );
    // and: the name stands
    assert_eq!(
        catalog
            .project_names(&[ProjectLink::from("project_1")])
            .await
            .expect("naming should succeed"),
        vec![named]
    );
}

pub async fn project_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand(
    catalog: &Committing<'_, impl NoticeCatalog>,
) {
    // given: a project's tombstone
    catalog
        .lay_project_tombstone(&ProjectLink::from("project_1"), at(3_000))
        .await
        .expect("laying a tombstone should succeed");
    // when: a transaction forgets it
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .forget_project_tombstones_laid_before(&mut transaction, at(5_000), 10)
        .await
        .expect("forgetting tombstones should succeed");

    // and: it is dropped
    drop(transaction);

    // then: it still stands to be forgotten
    assert_eq!(
        catalog
            .forget_project_tombstones_laid_before(at(5_000), 10)
            .await
            .expect("forgetting tombstones should succeed"),
        1
    );
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
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_notice_heard_of_late_is_not_remembered_again
        );
        $crate::catalog_conformance_case!(
            $workbench,
            notice_tombstones_are_forgotten_oldest_first_and_no_more_than_asked
        );
        $crate::catalog_conformance_case!(
            $workbench,
            project_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_account_s_open_notices_are_listed_for_it_alone
        );
        $crate::catalog_conformance_case!($workbench, the_newest_notice_comes_first);
        $crate::catalog_conformance_case!($workbench, a_dismissed_notice_is_listed_no_more);
        $crate::catalog_conformance_case!($workbench, a_version_heard_late_changes_nothing);
        $crate::catalog_conformance_case!(
            $workbench,
            every_role_and_who_changed_it_read_back_as_they_were_kept
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_person_is_named_as_last_told_and_nobody_unknown_is_named
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_keeps_its_newest_name_however_late_an_older_one_is_heard
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_deleted_project_s_notices_are_listed_no_more
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_deleted_project_s_name_is_forgotten_and_one_heard_late_is_not_kept
        );
        $crate::catalog_conformance_case!(
            $workbench,
            the_accounts_with_open_notices_about_a_project_are_named_once_each
        );
        $crate::catalog_conformance_case!(
            $workbench,
            notices_long_dismissed_long_noted_or_of_a_deleted_project_are_due_and_forgotten
        );
        $crate::catalog_conformance_case!(
            $workbench,
            old_project_tombstones_are_forgotten_oldest_first_and_no_more_than_asked
        );
    };
}
