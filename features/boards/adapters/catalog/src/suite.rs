use std::ops::Deref;

use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, IdeaLink, ProjectLink};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions, Tried};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: BoardCatalog;

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

impl<C: BoardCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn remember(&self, summary: &BoardSummary) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, summary)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget(&self, board: &BoardId, at: OffsetDateTime) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, board, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn holds(&self, board: BoardId, ideas: &[IdeaLink]) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .holds(transaction, board, ideas)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn lay_tombstone(
        &self,
        idea: &IdeaLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .lay_tombstone(transaction, idea, at)
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
            let done = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
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

pub fn a_summary(id: BoardId, project: &str) -> BoardSummary {
    BoardSummary {
        id,
        project: ProjectLink::from(project),
    }
}

pub async fn a_remembered_board_is_listed_in_its_project(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a board of a project
    let summary = a_summary(BoardId::generate(at(1_000)), "project_1");

    // when: it is remembered
    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    // then: the project lists it
    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![summary]);
}

pub async fn a_project_that_never_opened_a_board_lists_nothing(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // when: a project that never opened a board is listed
    let found = catalog
        .in_project(&ProjectLink::from("project_empty"))
        .await
        .expect("listing an unknown project should not fail");

    // then: it lists nothing
    assert!(
        found.is_empty(),
        "an empty listing is what tells the service to start one"
    );
}

pub async fn boards_of_other_projects_are_not_listed(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: a board of my project
    catalog
        .remember(&a_summary(BoardId::generate(at(1_000)), "project_mine"))
        .await
        .expect("remembering should succeed");

    // when: their project is listed
    let found = catalog
        .in_project(&ProjectLink::from("project_theirs"))
        .await
        .expect("listing should succeed");

    // then: it lists nothing
    assert!(found.is_empty());
}

pub async fn remembering_the_same_board_again_replaces_what_was_there(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a remembered board
    let id = BoardId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "project_1"))
        .await
        .expect("remembering should succeed");

    // when: it is remembered again
    catalog
        .remember(&a_summary(id, "project_1"))
        .await
        .expect("remembering again should succeed");

    // then: its project lists it once
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

pub async fn a_project_lists_its_boards_in_a_settled_order(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: boards of a project, remembered newest first
    let opened: Vec<BoardId> = (1..=6)
        .map(|nth| BoardId::generate(at(nth * 1_000)))
        .collect();

    for id in opened.iter().rev() {
        catalog
            .remember(&a_summary(*id, "project_1"))
            .await
            .expect("remembering should succeed");
    }

    // when: the project is listed
    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    // then: it lists them oldest first
    assert_eq!(
        found.into_iter().map(|board| board.id).collect::<Vec<_>>(),
        opened,
        "one board ships, but find-or-start must not pick a different one each time"
    );
}

pub async fn an_idea_nobody_pinned_is_on_no_board(catalog: &Committing<'_, impl BoardCatalog>) {
    // when: the boards holding an idea nobody pinned are asked for
    let found = catalog
        .boards_holding(&IdeaLink::from("idea_loose"))
        .await
        .expect("looking should succeed");

    // then: there are none
    assert!(
        found.is_empty(),
        "an unpinned idea is not an error, it is just not on a board"
    );
}

pub async fn a_pinned_idea_names_the_board_holding_it(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: a board
    let board = BoardId::generate(at(1_000));

    // when: it holds an idea
    catalog
        .holds(board, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");

    // then: the idea names it
    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![board]
    );
}

pub async fn every_idea_on_a_board_names_it(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: a board
    let board = BoardId::generate(at(1_000));
    // and: some ideas
    let ideas = ["idea_1", "idea_2", "idea_3"].map(IdeaLink::from);

    // when: it holds them
    catalog
        .holds(board, &ideas)
        .await
        .expect("indexing should succeed");

    // then: each names it
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

pub async fn an_idea_may_sit_on_more_than_one_board(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: two boards
    let earliest = BoardId::generate(at(1_000));
    let latest = BoardId::generate(at(2_000));
    // when: both hold an idea
    for board in [latest, earliest] {
        catalog
            .holds(board, &[IdeaLink::from("idea_1")])
            .await
            .expect("indexing should succeed");
    }

    // then: the idea names both, oldest first
    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![earliest, latest],
        "the model allows several boards, so the answer is a list in a settled order"
    );
}

pub async fn what_a_board_holds_is_replaced_not_added_to(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a board holding an idea
    let board = BoardId::generate(at(1_000));
    catalog
        .holds(board, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");

    // when: it is said to hold another instead
    catalog
        .holds(board, &[IdeaLink::from("idea_2")])
        .await
        .expect("indexing again should succeed");

    // then: the first names no board
    assert!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "the projector writes the whole set, so an unpinned idea falls out of the index"
    );
    // and: the second names it
    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_2"))
            .await
            .expect("looking should succeed"),
        vec![board]
    );
}

pub async fn one_board_letting_an_idea_go_leaves_the_others_holding_it(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: two boards holding an idea
    let keeping = BoardId::generate(at(1_000));
    let dropping = BoardId::generate(at(2_000));
    for board in [keeping, dropping] {
        catalog
            .holds(board, &[IdeaLink::from("idea_1")])
            .await
            .expect("indexing should succeed");
    }

    // when: one lets it go
    catalog
        .holds(dropping, &[])
        .await
        .expect("indexing should succeed");

    // then: the idea names only the other
    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![keeping],
        "an index kept in both directions must not forget the boards that still hold it"
    );
}

pub async fn a_forgotten_board_takes_its_pins_with_it(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: two boards of two projects holding an idea
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

    // when: one board is forgotten
    catalog
        .forget(&board, at(5_000))
        .await
        .expect("forgetting should succeed");

    // then: its project lists nothing
    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
    // and: the idea names only the other board
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

pub async fn forgetting_a_board_nobody_opened_is_harmless(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // expect: forgetting a board nobody opened succeeds
    catalog
        .forget(&BoardId::generate(at(1_000)), at(5_000))
        .await
        .expect("forgetting an unknown board should not fail");
}

fn an_idea(id: &str) -> IdeaLink {
    IdeaLink::from(id)
}

async fn held_though_tombstoned_by(
    catalog: &Committing<'_, impl BoardCatalog>,
    board: BoardId,
) -> Vec<IdeaLink> {
    catalog
        .held_though_tombstoned(board)
        .await
        .expect("looking for what is gone should succeed")
}

async fn tombstoned(catalog: &Committing<'_, impl BoardCatalog>, idea: &str) {
    tombstoned_at(catalog, idea, 1_000).await;
}

async fn tombstoned_at(catalog: &Committing<'_, impl BoardCatalog>, idea: &str, seconds: i64) {
    catalog
        .lay_tombstone(&an_idea(idea), at(seconds))
        .await
        .expect("laying a tombstone should succeed");
}

async fn pinned(catalog: &Committing<'_, impl BoardCatalog>, board: BoardId, ideas: &[&str]) {
    let ideas: Vec<IdeaLink> = ideas.iter().map(|idea| an_idea(idea)).collect();
    catalog
        .holds(board, &ideas)
        .await
        .expect("indexing should succeed");
}

pub async fn nothing_pinned_is_gone_until_marked(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: a board holding an idea
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1"]).await;

    // then: nothing it holds is gone
    assert!(held_though_tombstoned_by(catalog, board).await.is_empty());
}

pub async fn a_pinned_idea_marked_gone_is_named(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: a board holding two ideas
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1", "idea_2"]).await;

    // when: one of them is marked gone
    tombstoned(catalog, "idea_1").await;

    // then: the board holds that one though it is gone
    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_1")]
    );
}

pub async fn an_idea_marked_gone_before_it_was_pinned_is_named(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: an idea marked gone
    let board = BoardId::generate(at(1_000));
    tombstoned(catalog, "idea_1").await;

    // when: a board holds it
    pinned(catalog, board, &["idea_1"]).await;

    // then: the board holds it though it is gone
    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_1")],
        "the discard may be heard before the pin is indexed"
    );
}

pub async fn a_gone_idea_let_go_is_no_longer_named(catalog: &Committing<'_, impl BoardCatalog>) {
    // given: a board holding an idea marked gone
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1"]).await;
    tombstoned(catalog, "idea_1").await;

    // when: the board lets it go
    pinned(catalog, board, &[]).await;

    // then: the board holds nothing gone
    assert!(held_though_tombstoned_by(catalog, board).await.is_empty());
}

pub async fn marking_the_same_idea_gone_twice_is_harmless(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a board holding an idea
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_1"]).await;

    // when: the idea is marked gone twice
    tombstoned(catalog, "idea_1").await;
    tombstoned(catalog, "idea_1").await;

    // then: the board holds it once though it is gone
    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_1")]
    );
}

async fn forget_long_gone(
    catalog: &Committing<'_, impl BoardCatalog>,
    before_seconds: i64,
    at_most: u64,
) -> u64 {
    catalog
        .forget_tombstones_laid_before(at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

pub async fn a_forgotten_tombstone_names_nothing_and_a_later_one_still_does(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a board holding an idea gone long ago and one gone just now
    let board = BoardId::generate(at(1_000));
    pinned(catalog, board, &["idea_long_gone", "idea_just_gone"]).await;
    tombstoned_at(catalog, "idea_long_gone", 1_000).await;
    tombstoned_at(catalog, "idea_just_gone", 2_000).await;

    // when: the tombstones laid before just now are forgotten
    assert_eq!(forget_long_gone(catalog, 2_000, u64::MAX).await, 1);

    // then: the board holds only the one gone just now though it is gone
    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_just_gone")],
        "laid at the moment asked about is not before it"
    );
}

pub async fn an_idea_s_tombstone_is_laid_once_when_it_first_went(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: an idea marked gone twice, at different times
    tombstoned_at(catalog, "idea_1", 1_000).await;
    tombstoned_at(catalog, "idea_1", 5_000).await;

    // expect: its tombstone is forgotten as of the first time
    assert_eq!(
        forget_long_gone(catalog, 2_000, u64::MAX).await,
        1,
        "a discard heard again is the same discard"
    );
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a board holding three ideas, gone at different times
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

    // when: at most two tombstones are forgotten
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 2);

    // then: the two oldest are
    assert_eq!(
        held_though_tombstoned_by(catalog, board).await,
        vec![an_idea("idea_newest")]
    );
    // and: the next forgetting forgets the last
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 1);
}

async fn listed(catalog: &Committing<'_, impl BoardCatalog>) -> Vec<BoardSummary> {
    catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed")
}

pub async fn a_forgotten_board_heard_of_late_is_not_remembered_again(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a forgotten board
    let gone = a_summary(BoardId::generate(at(1_000)), "project_1");
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    catalog
        .forget(&gone.id, at(5_000))
        .await
        .expect("forgetting should succeed");

    // when: it is remembered again
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");

    // then: its project lists nothing
    assert!(
        listed(catalog).await.is_empty(),
        "what was forgotten stays forgotten, however late or new a summary of it comes"
    );
}

pub async fn board_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: three forgotten boards
    let (older, newer, latest) = (
        a_summary(BoardId::generate(at(1_000)), "project_1"),
        a_summary(BoardId::generate(at(2_000)), "project_1"),
        a_summary(BoardId::generate(at(3_000)), "project_1"),
    );
    for (summary, laid) in [(&newer, 6_000), (&older, 5_000), (&latest, 9_000)] {
        catalog
            .forget(&summary.id, at(laid))
            .await
            .expect("forgetting should succeed");
    }

    // when: at most one tombstone laid before a moment is forgotten
    let forgotten = catalog
        .forget_tombstones_laid_before(at(8_000), 1)
        .await
        .expect("forgetting tombstones should succeed");
    // and: all three boards are remembered again
    for summary in [&older, &newer, &latest] {
        catalog
            .remember(summary)
            .await
            .expect("remembering should succeed");
    }

    // then: one tombstone is forgotten
    assert_eq!(forgotten, 1);
    // and: only the oldest board is listed again
    assert_eq!(
        listed(catalog).await,
        vec![older],
        "only the oldest tombstone is gone, so only it can be remembered again"
    );
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a board holding an idea
    let kept = a_summary(BoardId::generate(at(1_000)), "project_1");
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");
    catalog
        .holds(kept.id, &[IdeaLink::from("idea_1")])
        .await
        .expect("holding should succeed");
    // and: a transaction that changes all of it
    let mut transaction = catalog.begun().await;
    let uncommitted = catalog.uncommitted();
    uncommitted
        .remember(
            &mut transaction,
            &a_summary(BoardId::generate(at(2_000)), "project_1"),
        )
        .await
        .expect("remembering should succeed");
    uncommitted
        .holds(&mut transaction, kept.id, &[IdeaLink::from("idea_2")])
        .await
        .expect("holding should succeed");
    uncommitted
        .lay_tombstone(&mut transaction, &IdeaLink::from("idea_1"), at(3_000))
        .await
        .expect("laying a tombstone should succeed");
    uncommitted
        .forget(&mut transaction, &kept.id, at(3_000))
        .await
        .expect("forgetting should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the board is listed as before
    assert_eq!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed"),
        vec![kept.clone()]
    );
    // and: the idea names it as before
    assert_eq!(
        catalog
            .boards_holding(&IdeaLink::from("idea_1"))
            .await
            .expect("asking should succeed"),
        vec![kept.id]
    );
    // and: nothing it holds is gone
    assert!(
        catalog
            .held_though_tombstoned(kept.id)
            .await
            .expect("asking should succeed")
            .is_empty()
    );
}

pub async fn board_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a forgotten board
    let gone = a_summary(BoardId::generate(at(1_000)), "project_1");
    catalog
        .forget(&gone.id, at(5_000))
        .await
        .expect("forgetting should succeed");
    // and: a transaction forgetting its tombstone
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .forget_tombstones_laid_before(&mut transaction, at(8_000), 10)
        .await
        .expect("forgetting tombstones should succeed");

    // when: the transaction is dropped
    drop(transaction);
    // and: the board is remembered again
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");

    // then: its project lists nothing
    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

pub async fn a_project_has_no_tombstone_until_its_deletion_is_laid(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: two projects
    let (deleted, kept) = (
        ProjectLink::from("project_1"),
        ProjectLink::from("project_2"),
    );
    // and: whether the one has a tombstone
    let had_one_before = catalog
        .has_project_tombstone(&deleted)
        .await
        .expect("asking should succeed");

    // when: its deletion is laid
    catalog
        .lay_project_tombstone(&deleted, at(5_000))
        .await
        .expect("laying a project's tombstone should succeed");

    // then: it had none before
    assert!(!had_one_before, "a project nobody deleted has no tombstone");
    // and: it has one now
    assert!(
        catalog
            .has_project_tombstone(&deleted)
            .await
            .expect("asking should succeed")
    );
    // and: the other project has none
    assert!(
        !catalog
            .has_project_tombstone(&kept)
            .await
            .expect("asking should succeed"),
        "one project's deletion buries no other"
    );
}

pub async fn project_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: three deleted projects
    let (older, newer, latest) = (
        ProjectLink::from("project_1"),
        ProjectLink::from("project_2"),
        ProjectLink::from("project_3"),
    );
    for (project, laid) in [(&newer, 6_000), (&older, 5_000), (&latest, 9_000)] {
        catalog
            .lay_project_tombstone(project, at(laid))
            .await
            .expect("laying a project's tombstone should succeed");
    }

    // when: at most one tombstone laid before a moment is forgotten
    let forgotten = catalog
        .forget_tombstones_laid_before(at(8_000), 1)
        .await
        .expect("forgetting tombstones should succeed");

    // then: only the oldest project's tombstone is forgotten
    let mut standing = Vec::new();
    for project in [&older, &newer, &latest] {
        standing.push(
            catalog
                .has_project_tombstone(project)
                .await
                .expect("asking should succeed"),
        );
    }
    assert_eq!(forgotten, 1);
    assert_eq!(
        standing,
        vec![false, true, true],
        "only the oldest tombstone is forgotten, and only one of those due"
    );
}

pub async fn a_project_tombstone_laid_in_a_transaction_that_is_dropped_is_not_there(
    catalog: &Committing<'_, impl BoardCatalog>,
) {
    // given: a project
    let project = ProjectLink::from("project_1");
    // when: its deletion is laid in a transaction that is dropped
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .lay_project_tombstone(&mut transaction, &project, at(5_000))
        .await
        .expect("laying a project's tombstone should succeed");
    drop(transaction);

    // then: it has no tombstone
    assert!(
        !catalog
            .has_project_tombstone(&project)
            .await
            .expect("asking should succeed"),
        "a deletion heard in a hearing that did not commit was not heard"
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
            board_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_board_heard_of_late_is_not_remembered_again
        );
        $crate::catalog_conformance_case!(
            $workbench,
            board_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
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
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_has_no_tombstone_until_its_deletion_is_laid
        );
        $crate::catalog_conformance_case!(
            $workbench,
            project_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_tombstone_laid_in_a_transaction_that_is_dropped_is_not_there
        );
    };
}
