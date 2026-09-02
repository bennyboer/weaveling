use boards_core::{BoardCatalog, BoardId, BoardSummary, PieceLink, ProjectLink};
use time::OffsetDateTime;

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: BoardId, project: &str) -> BoardSummary {
    BoardSummary {
        id,
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
        .remember(&a_summary(id, "project_1"))
        .await
        .expect("remembering again should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(
        found.len(),
        1,
        "a projection is keyed by the board, so a second write replaces rather than accrues"
    );
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

pub async fn a_piece_nobody_pinned_is_on_no_board(catalog: &impl BoardCatalog) {
    let found = catalog
        .boards_holding(&PieceLink::from("piece_loose"))
        .await
        .expect("looking should succeed");

    assert!(
        found.is_empty(),
        "an unpinned piece is not an error, it is just not on a board"
    );
}

pub async fn a_pinned_piece_names_the_board_holding_it(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));

    catalog
        .holds(board, &[PieceLink::from("piece_1")])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .boards_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed"),
        vec![board]
    );
}

pub async fn a_piece_may_sit_on_more_than_one_board(catalog: &impl BoardCatalog) {
    let earliest = BoardId::generate(at(1_000));
    let latest = BoardId::generate(at(2_000));
    for board in [latest, earliest] {
        catalog
            .holds(board, &[PieceLink::from("piece_1")])
            .await
            .expect("indexing should succeed");
    }

    assert_eq!(
        catalog
            .boards_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed"),
        vec![earliest, latest],
        "the model allows several boards, so the answer is a list in a settled order"
    );
}

pub async fn what_a_board_holds_is_replaced_not_added_to(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    catalog
        .holds(board, &[PieceLink::from("piece_1")])
        .await
        .expect("indexing should succeed");

    catalog
        .holds(board, &[PieceLink::from("piece_2")])
        .await
        .expect("indexing again should succeed");

    assert!(
        catalog
            .boards_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "the projector writes the whole set, so an unpinned piece falls out of the index"
    );
    assert_eq!(
        catalog
            .boards_holding(&PieceLink::from("piece_2"))
            .await
            .expect("looking should succeed"),
        vec![board]
    );
}

pub async fn one_board_letting_a_piece_go_leaves_the_others_holding_it(
    catalog: &impl BoardCatalog,
) {
    let keeping = BoardId::generate(at(1_000));
    let dropping = BoardId::generate(at(2_000));
    for board in [keeping, dropping] {
        catalog
            .holds(board, &[PieceLink::from("piece_1")])
            .await
            .expect("indexing should succeed");
    }

    catalog
        .holds(dropping, &[])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .boards_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed"),
        vec![keeping],
        "an index kept in both directions must not forget the boards that still hold it"
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
        $crate::catalog_conformance_case!($make_catalog, a_piece_nobody_pinned_is_on_no_board);
        $crate::catalog_conformance_case!($make_catalog, a_pinned_piece_names_the_board_holding_it);
        $crate::catalog_conformance_case!($make_catalog, a_piece_may_sit_on_more_than_one_board);
        $crate::catalog_conformance_case!(
            $make_catalog,
            what_a_board_holds_is_replaced_not_added_to
        );
        $crate::catalog_conformance_case!(
            $make_catalog,
            one_board_letting_a_piece_go_leaves_the_others_holding_it
        );
    };
}
