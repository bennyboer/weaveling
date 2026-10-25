use boards_core::{BoardCatalog, BoardId, BoardSummary, IdeaLink, ProjectLink};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: BoardCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}

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

pub async fn an_idea_nobody_pinned_is_on_no_board(catalog: &impl BoardCatalog) {
    let found = catalog
        .boards_holding(&IdeaLink::from("idea_loose"))
        .await
        .expect("looking should succeed");

    assert!(
        found.is_empty(),
        "an unpinned idea is not an error, it is just not on a board"
    );
}

pub async fn a_pinned_idea_names_the_board_holding_it(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));

    catalog
        .holds(board, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![board]
    );
}

pub async fn every_idea_on_a_board_names_it(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    let ideas = ["idea_1", "idea_2", "idea_3"].map(IdeaLink::from);

    catalog
        .holds(board, &ideas)
        .await
        .expect("indexing should succeed");

    for idea in &ideas {
        assert_eq!(
            catalog
                .boards_holding(idea)
                .await
                .expect("looking should succeed"),
            vec![board],
            "a board holds every idea it was given, not only the first"
        );
    }
}

pub async fn an_idea_may_sit_on_more_than_one_board(catalog: &impl BoardCatalog) {
    let earliest = BoardId::generate(at(1_000));
    let latest = BoardId::generate(at(2_000));
    for board in [latest, earliest] {
        catalog
            .holds(board, &[IdeaLink::from("idea_1")])
            .await
            .expect("indexing should succeed");
    }

    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![earliest, latest],
        "the model allows several boards, so the answer is a list in a settled order"
    );
}

pub async fn what_a_board_holds_is_replaced_not_added_to(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    catalog
        .holds(board, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");

    catalog
        .holds(board, &[IdeaLink::from("idea_2")])
        .await
        .expect("indexing again should succeed");

    assert!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "the projector writes the whole set, so an unpinned idea falls out of the index"
    );
    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_2"))
            .await
            .expect("looking should succeed"),
        vec![board]
    );
}

pub async fn one_board_letting_an_idea_go_leaves_the_others_holding_it(
    catalog: &impl BoardCatalog,
) {
    let keeping = BoardId::generate(at(1_000));
    let dropping = BoardId::generate(at(2_000));
    for board in [keeping, dropping] {
        catalog
            .holds(board, &[IdeaLink::from("idea_1")])
            .await
            .expect("indexing should succeed");
    }

    catalog
        .holds(dropping, &[])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![keeping],
        "an index kept in both directions must not forget the boards that still hold it"
    );
}

pub async fn a_forgotten_board_takes_its_pins_with_it(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    let elsewhere = BoardId::generate(at(2_000));
    catalog
        .remember(&a_summary(board, "project_1"))
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&a_summary(elsewhere, "project_2"))
        .await
        .expect("remembering should succeed");
    catalog
        .holds(board, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");
    catalog
        .holds(elsewhere, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");

    catalog
        .forget(&board)
        .await
        .expect("forgetting should succeed");

    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![elsewhere],
        "a forgotten board must take its pins with it, or the index keeps answering for a board \
         that is gone — and leave every other board's alone"
    );
}

pub async fn forgetting_a_board_nobody_opened_is_harmless(catalog: &impl BoardCatalog) {
    catalog
        .forget(&BoardId::generate(at(1_000)))
        .await
        .expect("forgetting an unknown board should not fail");
}

fn an_idea(id: &str) -> IdeaLink {
    IdeaLink::from(id)
}

async fn held_though_tombstoned_by(catalog: &impl BoardCatalog, board: BoardId) -> Vec<IdeaLink> {
    catalog
        .held_though_tombstoned(board)
        .await
        .expect("looking for what is gone should succeed")
}

async fn tombstoned(catalog: &impl BoardCatalog, idea: &str) {
    tombstoned_at(catalog, idea, 1_000).await;
}

async fn tombstoned_at(catalog: &impl BoardCatalog, idea: &str, seconds: i64) {
    catalog
        .lay_tombstone(&an_idea(idea), at(seconds))
        .await
        .expect("laying a tombstone should succeed");
}

async fn pinned(catalog: &impl BoardCatalog, board: BoardId, ideas: &[&str]) {
    let ideas: Vec<IdeaLink> = ideas.iter().map(|idea| an_idea(idea)).collect();
    catalog
        .holds(board, &ideas)
        .await
        .expect("indexing should succeed");
}

pub async fn nothing_pinned_is_gone_until_marked(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1"]).await;

    assert!(held_though_tombstoned_by(catalog, board).await.is_empty());
}

pub async fn a_pinned_idea_marked_gone_is_named(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1", "idea_2"]).await;

    tombstoned(catalog, "idea_1").await;

    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_1")]
    );
}

pub async fn an_idea_marked_gone_before_it_was_pinned_is_named(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    tombstoned(catalog, "idea_1").await;

    pinned(catalog, board, &["idea_1"]).await;

    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_1")],
        "the discard may be heard before the pin is indexed"
    );
}

pub async fn a_gone_idea_let_go_is_no_longer_named(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1"]).await;
    tombstoned(catalog, "idea_1").await;

    pinned(catalog, board, &[]).await;

    assert!(held_though_tombstoned_by(catalog, board).await.is_empty());
}

pub async fn marking_the_same_idea_gone_twice_is_harmless(catalog: &impl BoardCatalog) {
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1"]).await;

    tombstoned(catalog, "idea_1").await;
    tombstoned(catalog, "idea_1").await;

    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_1")]
    );
}

async fn forget_long_gone(catalog: &impl BoardCatalog, before_seconds: i64, at_most: u64) -> u64 {
    catalog
        .forget_tombstones_laid_before(at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

pub async fn a_forgotten_tombstone_names_nothing_and_a_later_one_still_does(
    catalog: &impl BoardCatalog,
) {
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_long_gone", "idea_just_gone"]).await;
    tombstoned_at(catalog, "idea_long_gone", 1_000).await;
    tombstoned_at(catalog, "idea_just_gone", 2_000).await;

    assert_eq!(forget_long_gone(catalog, 2_000, u64::MAX).await, 1);

    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_just_gone")],
        "laid at the moment asked about is not before it"
    );
}

pub async fn an_idea_s_tombstone_is_laid_once_when_it_first_went(catalog: &impl BoardCatalog) {
    tombstoned_at(catalog, "idea_1", 1_000).await;
    tombstoned_at(catalog, "idea_1", 5_000).await;

    assert_eq!(
        forget_long_gone(catalog, 2_000, u64::MAX).await,
        1,
        "a discard heard again is the same discard"
    );
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &impl BoardCatalog,
) {
    let board = BoardId::generate(at(1_000));
    pinned(
        catalog,
        board,
        &["idea_newest", "idea_oldest", "idea_older"],
    )
    .await;
    for (idea, seconds) in [
        ("idea_newest", 1_002),
        ("idea_oldest", 1_000),
        ("idea_older", 1_001),
    ] {
        tombstoned_at(catalog, idea, seconds).await;
    }

    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 2);

    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_newest")]
    );
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 1);
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
        $crate::catalog_conformance_case!($workbench, a_remembered_board_is_listed_in_its_project);
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_that_never_opened_a_board_lists_nothing
        );
        $crate::catalog_conformance_case!($workbench, boards_of_other_projects_are_not_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            remembering_the_same_board_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_lists_its_boards_in_a_settled_order
        );
        $crate::catalog_conformance_case!($workbench, an_idea_nobody_pinned_is_on_no_board);
        $crate::catalog_conformance_case!($workbench, a_forgotten_board_takes_its_pins_with_it);
        $crate::catalog_conformance_case!($workbench, forgetting_a_board_nobody_opened_is_harmless);
        $crate::catalog_conformance_case!($workbench, a_pinned_idea_names_the_board_holding_it);
        $crate::catalog_conformance_case!($workbench, an_idea_may_sit_on_more_than_one_board);
        $crate::catalog_conformance_case!($workbench, what_a_board_holds_is_replaced_not_added_to);
        $crate::catalog_conformance_case!(
            $workbench,
            one_board_letting_an_idea_go_leaves_the_others_holding_it
        );
        $crate::catalog_conformance_case!($workbench, every_idea_on_a_board_names_it);
        $crate::catalog_conformance_case!($workbench, nothing_pinned_is_gone_until_marked);
        $crate::catalog_conformance_case!($workbench, a_pinned_idea_marked_gone_is_named);
        $crate::catalog_conformance_case!(
            $workbench,
            an_idea_marked_gone_before_it_was_pinned_is_named
        );
        $crate::catalog_conformance_case!($workbench, a_gone_idea_let_go_is_no_longer_named);
        $crate::catalog_conformance_case!($workbench, marking_the_same_idea_gone_twice_is_harmless);
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_tombstone_names_nothing_and_a_later_one_still_does
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_idea_s_tombstone_is_laid_once_when_it_first_went
        );
        $crate::catalog_conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
    };
}
