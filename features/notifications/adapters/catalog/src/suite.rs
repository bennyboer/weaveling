use eventsourcing::{AccountId, Version};
use notifications_core::{About, NoticeCatalog, NoticeId, NoticeSummary, ProjectLink, Role};
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
    };
}
