use eventsourcing::Version;
use pieces_core::{PassageLink, PieceCatalog, PieceId, PieceSummary, PieceTitle, ProjectLink};
use time::OffsetDateTime;

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: PieceId, project: &str, title: &str) -> PieceSummary {
    PieceSummary {
        id,
        version: Version::of(1),
        project: ProjectLink::from(project),
        title: PieceTitle::new(title).expect("a plain title is fine"),
        passage: None,
    }
}

pub async fn a_remembered_piece_is_listed_in_its_project(catalog: &impl PieceCatalog) {
    let summary = a_summary(PieceId::generate(at(1_000)), "project_1", "The Loom");

    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![summary]);
}

pub async fn a_project_nobody_wrote_in_lists_nothing(catalog: &impl PieceCatalog) {
    let found = catalog
        .in_project(&ProjectLink::from("project_empty"))
        .await
        .expect("listing an unknown project should not fail");

    assert!(found.is_empty());
}

pub async fn pieces_of_other_projects_are_not_listed(catalog: &impl PieceCatalog) {
    catalog
        .remember(&a_summary(
            PieceId::generate(at(1_000)),
            "project_mine",
            "Mine",
        ))
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&a_summary(
            PieceId::generate(at(1_000)),
            "project_theirs",
            "Theirs",
        ))
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_mine"))
        .await
        .expect("listing should succeed");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title.as_str(), "Mine");
}

pub async fn remembering_the_same_piece_again_replaces_what_was_there(catalog: &impl PieceCatalog) {
    let id = PieceId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "project_1", "The Loom"))
        .await
        .expect("remembering should succeed");

    let mut later = a_summary(id, "project_1", "The Silent Loom");
    later.version = Version::of(2);
    later.passage = Some(PassageLink::from("passage_9"));
    catalog
        .remember(&later)
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![later], "a catalog holds one row per piece");
}

pub async fn a_forgotten_piece_is_no_longer_listed(catalog: &impl PieceCatalog) {
    let id = PieceId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "project_1", "The Loom"))
        .await
        .expect("remembering should succeed");

    catalog
        .forget(&id)
        .await
        .expect("forgetting should succeed");

    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

pub async fn forgetting_something_never_remembered_is_harmless(catalog: &impl PieceCatalog) {
    catalog
        .forget(&PieceId::generate(at(1_000)))
        .await
        .expect("forgetting an unknown piece should not fail");
}

pub async fn the_newest_piece_is_listed_first(catalog: &impl PieceCatalog) {
    let captured: Vec<PieceId> = (1..=6)
        .map(|nth| PieceId::generate(at(nth * 1_000)))
        .collect();

    for id in &captured {
        catalog
            .remember(&a_summary(*id, "project_1", "A piece"))
            .await
            .expect("remembering should succeed");
    }

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(
        found.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        captured.into_iter().rev().collect::<Vec<_>>(),
        "the idea an author had most recently should be the first they see"
    );
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($make_catalog:expr, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            $crate::suite::$case(&$make_catalog).await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($make_catalog:expr) => {
        $crate::catalog_conformance_case!(
            $make_catalog,
            a_remembered_piece_is_listed_in_its_project
        );
        $crate::catalog_conformance_case!($make_catalog, a_project_nobody_wrote_in_lists_nothing);
        $crate::catalog_conformance_case!($make_catalog, pieces_of_other_projects_are_not_listed);
        $crate::catalog_conformance_case!(
            $make_catalog,
            remembering_the_same_piece_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!($make_catalog, a_forgotten_piece_is_no_longer_listed);
        $crate::catalog_conformance_case!(
            $make_catalog,
            forgetting_something_never_remembered_is_harmless
        );
        $crate::catalog_conformance_case!($make_catalog, the_newest_piece_is_listed_first);
    };
}
