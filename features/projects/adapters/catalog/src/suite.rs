use eventsourcing::Version;
use projects_core::{ProjectCatalog, ProjectId, ProjectName, ProjectSummary};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: ProjectCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
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
    }
}

pub async fn a_remembered_project_is_listed(catalog: &impl ProjectCatalog) {
    let summary = a_summary(ProjectId::generate(at(1_000)), "The Weaver's Apprentice");

    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    let found = catalog.all().await.expect("listing should succeed");

    assert_eq!(found, vec![summary]);
}

pub async fn an_empty_catalog_lists_nothing(catalog: &impl ProjectCatalog) {
    let found = catalog
        .all()
        .await
        .expect("listing an empty catalog should not fail");

    assert!(found.is_empty());
}

pub async fn remembering_the_same_project_again_replaces_what_was_there(
    catalog: &impl ProjectCatalog,
) {
    let id = ProjectId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "Working Title"))
        .await
        .expect("remembering should succeed");

    let mut later = a_summary(id, "The Weaver's Apprentice");
    later.version = Version::of(2);
    later.updated_at = at(2_000);
    catalog
        .remember(&later)
        .await
        .expect("remembering should succeed");

    let found = catalog.all().await.expect("listing should succeed");

    assert_eq!(found, vec![later], "a catalog holds one row per project");
}

pub async fn a_forgotten_project_is_no_longer_listed(catalog: &impl ProjectCatalog) {
    let id = ProjectId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "Abandoned"))
        .await
        .expect("remembering should succeed");

    catalog
        .forget(&id)
        .await
        .expect("forgetting should succeed");

    assert!(
        catalog
            .all()
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

pub async fn forgetting_something_never_remembered_is_harmless(catalog: &impl ProjectCatalog) {
    catalog
        .forget(&ProjectId::generate(at(1_000)))
        .await
        .expect("forgetting an unknown project should not fail");
}

pub async fn the_newest_project_is_listed_first(catalog: &impl ProjectCatalog) {
    let started: Vec<ProjectId> = (1..=6)
        .map(|nth| ProjectId::generate(at(nth * 1_000)))
        .collect();

    for id in &started {
        catalog
            .remember(&a_summary(*id, "A project"))
            .await
            .expect("remembering should succeed");
    }

    let found = catalog.all().await.expect("listing should succeed");

    assert_eq!(
        found.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        started.into_iter().rev().collect::<Vec<_>>(),
        "the project an author started most recently should be the first they see"
    );
}

pub async fn the_moments_a_project_carries_survive_the_round_trip(catalog: &impl ProjectCatalog) {
    let mut summary = a_summary(ProjectId::generate(at(1_000)), "Timestamped");
    summary.created_at = at(1_700_000_000);
    summary.updated_at = at(1_700_086_400);

    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    let found = catalog.all().await.expect("listing should succeed");

    assert_eq!(
        found
            .first()
            .map(|listed| (listed.created_at, listed.updated_at)),
        Some((at(1_700_000_000), at(1_700_086_400))),
        "the listing shows an author when they last touched a project, so the moments must not \
         be rounded or dropped on the way through"
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
        $crate::catalog_conformance_case!($workbench, a_remembered_project_is_listed);
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
            the_moments_a_project_carries_survive_the_round_trip
        );
    };
}
