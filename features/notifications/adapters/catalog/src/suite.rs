use std::slice;

use eventsourcing::{AccountId, Version};
use notifications_core::{
    About, NoticeCatalog, NoticeId, NoticeSummary, Person, ProjectLink, ProjectName, Role,
};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: NoticeCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
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
        dismissed,
    }
}

async fn open_for(catalog: &impl NoticeCatalog, account: &str) -> Vec<NoticeSummary> {
    catalog
        .open_for(&AccountId::from(account))
        .await
        .expect("listing should succeed")
}

async fn remembered(catalog: &impl NoticeCatalog, summary: &NoticeSummary) {
    catalog
        .remember(summary)
        .await
        .expect("remembering should succeed");
}

pub async fn an_account_s_open_notices_are_listed_for_it_alone(catalog: &impl NoticeCatalog) {
    let bo_s = a_summary("account_bo", 1_000, 1, false);
    remembered(catalog, &bo_s).await;
    remembered(catalog, &a_summary("account_cy", 1_000, 1, false)).await;

    assert_eq!(open_for(catalog, "account_bo").await, vec![bo_s]);
    assert!(open_for(catalog, "account_dee").await.is_empty());
}

pub async fn the_newest_notice_comes_first(catalog: &impl NoticeCatalog) {
    let older = a_summary("account_bo", 1_000, 1, false);
    let newer = a_summary("account_bo", 2_000, 1, false);
    remembered(catalog, &older).await;
    remembered(catalog, &newer).await;

    assert_eq!(open_for(catalog, "account_bo").await, vec![newer, older]);
}

pub async fn a_dismissed_notice_is_listed_no_more(catalog: &impl NoticeCatalog) {
    let noted = a_summary("account_bo", 1_000, 1, false);
    remembered(catalog, &noted).await;

    remembered(
        catalog,
        &NoticeSummary {
            version: Version::of(2),
            dismissed: true,
            ..noted
        },
    )
    .await;

    assert!(open_for(catalog, "account_bo").await.is_empty());
}

pub async fn a_version_heard_late_changes_nothing(catalog: &impl NoticeCatalog) {
    let noted = a_summary("account_bo", 1_000, 1, false);
    let dismissed = NoticeSummary {
        version: Version::of(2),
        dismissed: true,
        ..noted.clone()
    };
    remembered(catalog, &dismissed).await;

    remembered(catalog, &noted).await;

    assert!(
        open_for(catalog, "account_bo").await.is_empty(),
        "a notice dismissed stays dismissed, however late its noting is heard"
    );
}

pub async fn every_role_and_who_changed_it_read_back_as_they_were_kept(
    catalog: &impl NoticeCatalog,
) {
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

    assert_eq!(open_for(catalog, "account_bo").await, kept);
}

pub async fn a_person_is_named_as_last_told_and_nobody_unknown_is_named(
    catalog: &impl NoticeCatalog,
) {
    for name in ["Ada", "Ada Lovelace"] {
        catalog
            .remember_person(&Person {
                account: AccountId::from("account_ada"),
                name: name.to_owned(),
            })
            .await
            .expect("remembering should succeed");
    }

    let named = catalog
        .people(&[
            AccountId::from("account_ada"),
            AccountId::from("account_nobody"),
        ])
        .await
        .expect("asking should succeed");

    assert_eq!(
        named,
        vec![Person {
            account: AccountId::from("account_ada"),
            name: "Ada Lovelace".to_owned(),
        }]
    );
}

fn named(version: u64, name: &str) -> ProjectName {
    ProjectName {
        project: ProjectLink::from("project_1"),
        version: Version::of(version),
        name: name.to_owned(),
    }
}

pub async fn a_project_keeps_its_newest_name_however_late_an_older_one_is_heard(
    catalog: &impl NoticeCatalog,
) {
    for told in [
        named(1, "Shared"),
        named(3, "Shared Again"),
        named(2, "Between"),
    ] {
        catalog
            .remember_project_name(&told)
            .await
            .expect("remembering should succeed");
    }

    let names = catalog
        .project_names(&[
            ProjectLink::from("project_1"),
            ProjectLink::from("project_unknown"),
        ])
        .await
        .expect("asking should succeed");

    assert_eq!(names, vec![named(3, "Shared Again")]);
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

pub async fn a_deleted_project_s_notices_are_listed_no_more(catalog: &impl NoticeCatalog) {
    let kept = in_project(a_summary("account_bo", 2_000, 1, false), "project_2");
    remembered(
        catalog,
        &in_project(a_summary("account_bo", 1_000, 1, false), "project_1"),
    )
    .await;
    remembered(catalog, &kept).await;

    for _ in 0..2 {
        catalog
            .lay_project_tombstone(&ProjectLink::from("project_1"), at(3_000))
            .await
            .expect("laying a tombstone should succeed however often");
    }

    assert_eq!(open_for(catalog, "account_bo").await, vec![kept]);
}

pub async fn a_deleted_project_s_name_is_forgotten_and_one_heard_late_is_not_kept(
    catalog: &impl NoticeCatalog,
) {
    let project = ProjectLink::from("project_1");
    catalog
        .remember_project_name(&named(1, "Shared"))
        .await
        .expect("remembering should succeed");

    catalog
        .lay_project_tombstone(&project, at(3_000))
        .await
        .expect("laying a tombstone should succeed");
    catalog
        .remember_project_name(&named(2, "Renamed meanwhile"))
        .await
        .expect("a name heard late is no failure");

    assert!(
        catalog
            .project_names(slice::from_ref(&project))
            .await
            .expect("asking should succeed")
            .is_empty()
    );
    assert!(
        catalog
            .has_project_tombstone(&project)
            .await
            .expect("asking should succeed")
    );
    assert!(
        !catalog
            .has_project_tombstone(&ProjectLink::from("project_2"))
            .await
            .expect("asking should succeed")
    );
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
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
    };
}
