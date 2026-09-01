use boards_core::{BoardCatalog, BoardId, BoardSummary, ProjectLink};
use eventsourcing::Version;
use time::OffsetDateTime;

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: BoardId, project: &str) -> BoardSummary {
    BoardSummary {
        id,
        version: Version::of(1),
        project: ProjectLink::from(project),
    }
}

pub async fn a_remembered_board_is_listed_in_its_project(catalog: &impl BoardCatalog) {
    let summary = a_summary(BoardId::generate(at(1_000)), "project_1");

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

pub async fn a_project_that_never_opened_a_board_lists_nothing(catalog: &impl BoardCatalog) {
    let found = catalog
        .in_project(&ProjectLink::from("project_empty"))
        .await
        .expect("listing an unknown project should not fail");

    assert!(
        found.is_empty(),
        "an empty listing is what tells the service to start one"
    );
}

pub async fn boards_of_other_projects_are_not_listed(catalog: &impl BoardCatalog) {
    catalog
        .remember(&a_summary(BoardId::generate(at(1_000)), "project_mine"))
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_theirs"))
        .await
        .expect("listing should succeed");

    assert!(found.is_empty());
}

pub async fn remembering_the_same_board_again_replaces_what_was_there(catalog: &impl BoardCatalog) {
    let id = BoardId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "project_1"))
        .await
        .expect("remembering should succeed");

    catalog
        .remember(&BoardSummary {
            version: Version::of(9),
            ..a_summary(id, "project_1")
        })
        .await
        .expect("remembering again should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found.len(), 1, "a projection replaces, it does not accrue");
    assert_eq!(found[0].version, Version::of(9));
}

pub async fn a_project_lists_its_boards_in_a_settled_order(catalog: &impl BoardCatalog) {
    let opened: Vec<BoardId> = (1..=6)
        .map(|nth| BoardId::generate(at(nth * 1_000)))
        .collect();

    for id in opened.iter().rev() {
        catalog
            .remember(&a_summary(*id, "project_1"))
            .await
            .expect("remembering should succeed");
    }

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(
        found.into_iter().map(|board| board.id).collect::<Vec<_>>(),
        opened,
        "one board ships, but find-or-start must not pick a different one each time"
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
            a_remembered_board_is_listed_in_its_project
        );
        $crate::catalog_conformance_case!(
            $make_catalog,
            a_project_that_never_opened_a_board_lists_nothing
        );
        $crate::catalog_conformance_case!($make_catalog, boards_of_other_projects_are_not_listed);
        $crate::catalog_conformance_case!(
            $make_catalog,
            remembering_the_same_board_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!(
            $make_catalog,
            a_project_lists_its_boards_in_a_settled_order
        );
    };
}
