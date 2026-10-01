use eventsourcing::{Agent, AgentId, Aggregate, AggregateId, Event, EventMetadata, Version};
use time::{Duration, OffsetDateTime};

use crate::board::{
    Board, BoardCommand, BoardError, BoardEvent, IdeaLink, KIND, PositionedIdea, ProjectLink,
};
use crate::size::Size;
use crate::spot::Spot;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_metadata(version: u64) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("board_1"),
        kind: KIND,
        version: Version::of(version),
        agent: an_author(),
        occurred_at: at(1_000),
        is_snapshot: false,
    }
}

fn a_idea(named: &str) -> IdeaLink {
    IdeaLink::from(named)
}

fn a_started_board() -> Board {
    let started = BoardEvent::Started {
        project: ProjectLink::from("project_1"),
    };

    Board::from_first(&started, &a_metadata(1)).expect("a start should raise a board")
}

fn pinned_order(board: &Board) -> Vec<IdeaLink> {
    board.ideas().into_iter().map(|held| held.idea).collect()
}

fn a_board_holding(idea: &IdeaLink, at_spot: Spot) -> Board {
    let mut board = a_started_board();
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: idea.clone(),
            at: at_spot,
            size: Size::CARD,
        },
        &a_metadata(2),
    );

    board
}

#[test]
fn a_board_begins_by_being_started_for_a_project() {
    let events = Board::begin(
        BoardCommand::Start {
            project: ProjectLink::from("project_1"),
        },
        &an_author(),
    )
    .expect("starting should succeed");

    assert_eq!(
        events,
        vec![BoardEvent::Started {
            project: ProjectLink::from("project_1"),
        }]
    );
}

#[test]
fn nothing_can_be_pinned_before_the_board_exists() {
    let refused = Board::begin(
        BoardCommand::Pin {
            idea: a_idea("idea_1"),
            at: Spot::ORIGIN,
            size: Size::CARD,
        },
        &an_author(),
    );

    assert_eq!(refused, Err(BoardError::NotStartedYet));
}

#[test]
fn a_board_cannot_be_started_twice() {
    let board = a_started_board();

    let refused = board.decide(
        BoardCommand::Start {
            project: ProjectLink::from("project_2"),
        },
        &an_author(),
    );

    assert_eq!(refused, Err(BoardError::AlreadyStarted));
}

#[test]
fn a_started_board_holds_nothing_yet() {
    assert!(a_started_board().ideas().is_empty());
}

#[test]
fn pinning_a_idea_puts_it_where_it_was_dropped() {
    let board = a_started_board();
    let idea = a_idea("idea_1");

    let events = board
        .decide(
            BoardCommand::Pin {
                idea: idea.clone(),
                at: Spot::at(120, -40),
                size: Size::CARD,
            },
            &an_author(),
        )
        .expect("pinning should succeed");

    assert_eq!(
        events,
        vec![BoardEvent::IdeaPinned {
            idea,
            at: Spot::at(120, -40),
            size: Size::CARD,
        }]
    );
}

#[test]
fn a_pinned_idea_can_be_found_at_its_spot() {
    let idea = a_idea("idea_1");

    let board = a_board_holding(&idea, Spot::at(120, -40));

    assert_eq!(board.spot_of(&idea), Some(Spot::at(120, -40)));
    assert_eq!(
        board.ideas(),
        [PositionedIdea {
            idea,
            spot: Spot::at(120, -40),
            size: Size::CARD,
        }]
    );
}

#[test]
fn the_same_idea_cannot_be_pinned_twice() {
    let idea = a_idea("idea_1");
    let board = a_board_holding(&idea, Spot::ORIGIN);

    let refused = board.decide(
        BoardCommand::Pin {
            idea,
            at: Spot::at(9, 9),
            size: Size::CARD,
        },
        &an_author(),
    );

    assert_eq!(
        refused,
        Err(BoardError::AlreadyPinned),
        "a second pin is a move, and the caller should say so"
    );
}

#[test]
fn moving_a_idea_takes_it_to_the_new_spot() {
    let idea = a_idea("idea_1");
    let mut board = a_board_holding(&idea, Spot::at(10, 10));

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea: idea.clone(),
                to: Some(Spot::at(300, 20)),
                size: None,
            },
            &an_author(),
        )
        .expect("moving should succeed");
    board.apply(&events[0], &a_metadata(3));

    assert_eq!(board.spot_of(&idea), Some(Spot::at(300, 20)));
}

#[test]
fn moving_a_idea_nowhere_is_not_a_move() {
    let idea = a_idea("idea_1");
    let board = a_board_holding(&idea, Spot::at(10, 10));

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea,
                to: Some(Spot::at(10, 10)),
                size: None,
            },
            &an_author(),
        )
        .expect("moving should succeed");

    assert!(
        events.is_empty(),
        "a drag that ends where it began must not fill the log"
    );
}

#[test]
fn a_idea_that_is_not_on_the_board_cannot_be_moved() {
    let board = a_started_board();

    let refused = board.decide(
        BoardCommand::Reshape {
            idea: a_idea("idea_1"),
            to: Some(Spot::ORIGIN),
            size: None,
        },
        &an_author(),
    );

    assert_eq!(refused, Err(BoardError::NotPinned));
}

#[test]
fn unpinning_takes_a_idea_off_the_board() {
    let idea = a_idea("idea_1");
    let mut board = a_board_holding(&idea, Spot::at(10, 10));

    let events = board
        .decide(BoardCommand::Unpin { idea: idea.clone() }, &an_author())
        .expect("unpinning should succeed");
    board.apply(&events[0], &a_metadata(3));

    assert_eq!(board.spot_of(&idea), None);
    assert!(board.ideas().is_empty());
}

#[test]
fn a_idea_that_is_not_on_the_board_cannot_be_unpinned() {
    let board = a_started_board();

    let refused = board.decide(
        BoardCommand::Unpin {
            idea: a_idea("idea_1"),
        },
        &an_author(),
    );

    assert_eq!(refused, Err(BoardError::NotPinned));
}

#[test]
fn unpinning_one_idea_leaves_the_others_where_they_are() {
    let staying = a_idea("idea_1");
    let going = a_idea("idea_2");
    let mut board = a_board_holding(&staying, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: going.clone(),
            at: Spot::at(20, 20),
            size: Size::CARD,
        },
        &a_metadata(3),
    );

    board.apply(&BoardEvent::IdeaUnpinned { idea: going }, &a_metadata(4));

    assert_eq!(board.spot_of(&staying), Some(Spot::at(10, 10)));
}

#[test]
fn ideas_keep_the_order_they_were_pinned_in() {
    let first = a_idea("idea_1");
    let second = a_idea("idea_2");
    let mut board = a_board_holding(&first, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: second.clone(),
            at: Spot::at(20, 20),
            size: Size::CARD,
        },
        &a_metadata(3),
    );

    assert_eq!(
        board
            .ideas()
            .into_iter()
            .map(|held| held.idea)
            .collect::<Vec<_>>(),
        vec![first, second],
        "the order ideas come back in is the order they stack on the board"
    );
}

#[test]
fn moving_a_idea_does_not_restack_the_board() {
    let first = a_idea("idea_1");
    let second = a_idea("idea_2");
    let mut board = a_board_holding(&first, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: second.clone(),
            at: Spot::at(20, 20),
            size: Size::CARD,
        },
        &a_metadata(3),
    );

    board.apply(
        &BoardEvent::IdeaMoved {
            idea: first.clone(),
            to: Spot::at(99, 99),
        },
        &a_metadata(4),
    );

    assert_eq!(
        board
            .ideas()
            .into_iter()
            .map(|held| held.idea)
            .collect::<Vec<_>>(),
        vec![first, second],
        "a drag must not send a card to the front or the back"
    );
}

#[test]
fn unpinning_a_idea_leaves_the_rest_in_order() {
    let first = a_idea("idea_1");
    let going = a_idea("idea_2");
    let third = a_idea("idea_3");
    let last = a_idea("idea_4");
    let mut board = a_board_holding(&first, Spot::at(10, 10));
    for (idea, version) in [(&going, 3), (&third, 4), (&last, 5)] {
        board.apply(
            &BoardEvent::IdeaPinned {
                idea: idea.clone(),
                at: Spot::at(20, 20),
                size: Size::CARD,
            },
            &a_metadata(version),
        );
    }

    board.apply(&BoardEvent::IdeaUnpinned { idea: going }, &a_metadata(6));

    assert_eq!(
        pinned_order(&board),
        vec![first, third, last],
        "taking a card off the board must not shuffle the ones left, so the gap closes rather than \
         being filled from the end"
    );
}

#[test]
fn the_board_does_not_ask_whether_the_idea_exists() {
    let board = a_started_board();

    let pinned = board.decide(
        BoardCommand::Pin {
            idea: a_idea("idea_that_was_discarded"),
            at: Spot::ORIGIN,
            size: Size::CARD,
        },
        &an_author(),
    );

    assert!(
        pinned.is_ok(),
        "a board may not reach into ideas, so a dangling positioned is the client's to tolerate"
    );
}

#[test]
fn a_board_survives_on_its_snapshot_alone() {
    let idea = a_idea("idea_1");
    let board = a_board_holding(&idea, Spot::at(7, 8));

    let snapshot = board.snapshot();
    let recovered =
        Board::from_first(&snapshot, &a_metadata(9)).expect("a snapshot should restore");

    assert_eq!(recovered, board);
}

#[test]
fn a_snapshot_says_it_is_one() {
    assert!(a_started_board().snapshot().is_snapshot());
}

#[test]
fn a_pinned_idea_keeps_the_size_it_was_given() {
    let board = a_started_board();
    let idea = a_idea("idea_1");

    let events = board
        .decide(
            BoardCommand::Pin {
                idea: idea.clone(),
                at: Spot::at(120, -40),
                size: Size::of(400, 90),
            },
            &an_author(),
        )
        .expect("pinning should succeed");

    assert_eq!(
        events,
        vec![BoardEvent::IdeaPinned {
            idea,
            at: Spot::at(120, -40),
            size: Size::of(400, 90),
        }]
    );
}

#[test]
fn a_card_without_extent_cannot_be_pinned() {
    let board = a_started_board();

    for shapeless in [Size::of(0, 84), Size::of(168, 0), Size::of(-1, -1)] {
        assert_eq!(
            board.decide(
                BoardCommand::Pin {
                    idea: a_idea("idea_1"),
                    at: Spot::ORIGIN,
                    size: shapeless,
                },
                &an_author(),
            ),
            Err(BoardError::Shapeless),
            "{shapeless} should not be a card"
        );
    }
}

#[test]
fn resizing_a_idea_gives_it_the_new_extent() {
    let idea = a_idea("idea_1");
    let mut board = a_board_holding(&idea, Spot::at(10, 10));

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea: idea.clone(),
                to: None,
                size: Some(Size::of(400, 90)),
            },
            &an_author(),
        )
        .expect("resizing should succeed");
    board.apply(&events[0], &a_metadata(3));

    assert_eq!(
        events,
        vec![BoardEvent::IdeaResized {
            idea: idea.clone(),
            to: Size::of(400, 90),
        }]
    );
    assert_eq!(board.size_of(&idea), Some(Size::of(400, 90)));
    assert_eq!(
        board.spot_of(&idea),
        Some(Spot::at(10, 10)),
        "resizing from a corner that does not move must leave the spot alone"
    );
}

#[test]
fn dragging_an_edge_both_moves_and_resizes_in_one_go() {
    let idea = a_idea("idea_1");
    let mut board = a_board_holding(&idea, Spot::at(100, 100));

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea: idea.clone(),
                to: Some(Spot::at(60, 100)),
                size: Some(Size::of(208, 84)),
            },
            &an_author(),
        )
        .expect("reshaping should succeed");
    for (nth, event) in events.iter().enumerate() {
        board.apply(event, &a_metadata(3 + nth as u64));
    }

    assert_eq!(
        events,
        vec![
            BoardEvent::IdeaMoved {
                idea: idea.clone(),
                to: Spot::at(60, 100),
            },
            BoardEvent::IdeaResized {
                idea: idea.clone(),
                to: Size::of(208, 84),
            },
        ],
        "one gesture, two facts, and the log should say both"
    );
    assert_eq!(board.spot_of(&idea), Some(Spot::at(60, 100)));
    assert_eq!(board.size_of(&idea), Some(Size::of(208, 84)));
}

#[test]
fn reshaping_a_idea_into_the_shape_it_already_has_is_not_a_change() {
    let idea = a_idea("idea_1");
    let board = a_board_holding(&idea, Spot::at(10, 10));

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea,
                to: Some(Spot::at(10, 10)),
                size: Some(Size::CARD),
            },
            &an_author(),
        )
        .expect("reshaping should succeed");

    assert!(
        events.is_empty(),
        "a gesture that changes nothing must not fill the log"
    );
}

#[test]
fn a_card_cannot_be_resized_into_nothing() {
    let idea = a_idea("idea_1");
    let board = a_board_holding(&idea, Spot::at(10, 10));

    let refused = board.decide(
        BoardCommand::Reshape {
            idea,
            to: None,
            size: Some(Size::of(168, 0)),
        },
        &an_author(),
    );

    assert_eq!(refused, Err(BoardError::Shapeless));
}

#[test]
fn a_idea_that_is_not_on_the_board_cannot_be_reshaped() {
    let board = a_started_board();

    let refused = board.decide(
        BoardCommand::Reshape {
            idea: a_idea("idea_1"),
            to: None,
            size: Some(Size::of(400, 90)),
        },
        &an_author(),
    );

    assert_eq!(refused, Err(BoardError::NotPinned));
}

#[test]
fn a_snapshot_remembers_how_big_each_card_was() {
    let idea = a_idea("idea_1");
    let mut board = a_board_holding(&idea, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaResized {
            idea: idea.clone(),
            to: Size::of(400, 90),
        },
        &a_metadata(3),
    );

    let taken = board.snapshot();
    let regrown =
        Board::from_first(&taken, &a_metadata(4)).expect("a snapshot should raise a board");

    assert_eq!(regrown.size_of(&idea), Some(Size::of(400, 90)));
    assert_eq!(regrown.spot_of(&idea), Some(Spot::at(10, 10)));
}

#[test]
fn moving_a_idea_brings_it_to_the_front() {
    let under = a_idea("idea_1");
    let over = a_idea("idea_2");
    let mut board = a_board_holding(&under, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: over.clone(),
            at: Spot::at(20, 20),
            size: Size::CARD,
        },
        &a_metadata(3),
    );
    assert_eq!(pinned_order(&board), vec![under.clone(), over.clone()]);

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea: under.clone(),
                to: Some(Spot::at(30, 30)),
                size: None,
            },
            &an_author(),
        )
        .expect("moving should succeed");
    for (nth, event) in events.iter().enumerate() {
        board.apply(event, &a_metadata(4 + nth as u64));
    }

    assert_eq!(
        events,
        vec![
            BoardEvent::IdeaMoved {
                idea: under.clone(),
                to: Spot::at(30, 30),
            },
            BoardEvent::IdeaRaised {
                idea: under.clone(),
            },
        ]
    );
    assert_eq!(
        pinned_order(&board),
        vec![over, under],
        "the card you just touched should not hide behind one pinned later"
    );
}

#[test]
fn moving_the_card_that_is_already_in_front_does_not_raise_it_again() {
    let under = a_idea("idea_1");
    let over = a_idea("idea_2");
    let mut board = a_board_holding(&under, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: over.clone(),
            at: Spot::at(20, 20),
            size: Size::CARD,
        },
        &a_metadata(3),
    );

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea: over,
                to: Some(Spot::at(40, 40)),
                size: None,
            },
            &an_author(),
        )
        .expect("moving should succeed");

    assert_eq!(
        events.len(),
        1,
        "raising what is already on top says nothing"
    );
    assert!(matches!(events[0], BoardEvent::IdeaMoved { .. }));
}

#[test]
fn resizing_a_idea_leaves_the_stack_alone() {
    let under = a_idea("idea_1");
    let over = a_idea("idea_2");
    let mut board = a_board_holding(&under, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: over.clone(),
            at: Spot::at(20, 20),
            size: Size::CARD,
        },
        &a_metadata(3),
    );

    let events = board
        .decide(
            BoardCommand::Reshape {
                idea: under.clone(),
                to: None,
                size: Some(Size::of(400, 90)),
            },
            &an_author(),
        )
        .expect("resizing should succeed");
    for (nth, event) in events.iter().enumerate() {
        board.apply(event, &a_metadata(4 + nth as u64));
    }

    assert_eq!(
        pinned_order(&board),
        vec![under, over],
        "stretching a card is not the same as reaching for it"
    );
}

#[test]
fn a_snapshot_remembers_the_order_the_cards_are_stacked_in() {
    let under = a_idea("idea_1");
    let over = a_idea("idea_2");
    let mut board = a_board_holding(&under, Spot::at(10, 10));
    board.apply(
        &BoardEvent::IdeaPinned {
            idea: over.clone(),
            at: Spot::at(20, 20),
            size: Size::CARD,
        },
        &a_metadata(3),
    );
    board.apply(
        &BoardEvent::IdeaRaised {
            idea: under.clone(),
        },
        &a_metadata(4),
    );

    let regrown = Board::from_first(&board.snapshot(), &a_metadata(5))
        .expect("a snapshot should raise a board");

    assert_eq!(pinned_order(&regrown), vec![over, under]);
}

#[test]
fn a_board_can_be_discarded() {
    let board = a_started_board();

    let events = board
        .decide(BoardCommand::Discard, &an_author())
        .expect("discarding should succeed");

    assert_eq!(events, vec![BoardEvent::Discarded]);
}

#[test]
fn a_discarded_board_refuses_everything() {
    let mut board = a_board_holding(&a_idea("idea_1"), Spot::ORIGIN);
    board.apply(&BoardEvent::Discarded, &a_metadata(9));

    for command in [
        BoardCommand::Start {
            project: ProjectLink::from("project_1"),
        },
        BoardCommand::Pin {
            idea: a_idea("idea_2"),
            at: Spot::ORIGIN,
            size: Size::CARD,
        },
        BoardCommand::Reshape {
            idea: a_idea("idea_1"),
            to: Some(Spot::at(10, 10)),
            size: None,
        },
        BoardCommand::Unpin {
            idea: a_idea("idea_1"),
        },
        BoardCommand::Discard,
    ] {
        assert_eq!(
            board.decide(command, &an_author()),
            Err(BoardError::Discarded),
            "a board is discarded when its project is deleted, so anything still holding a \
             reference to it must be refused rather than quietly allowed"
        );
    }
}

#[test]
fn a_discarded_board_still_says_what_it_held() {
    let mut board = a_board_holding(&a_idea("idea_1"), Spot::ORIGIN);
    board.apply(&BoardEvent::Discarded, &a_metadata(9));

    assert!(board.is_discarded());
    assert_eq!(
        pinned_order(&board),
        vec![a_idea("idea_1")],
        "the stream is the audit log, so a discarded board still reads back what was on it"
    );
}

#[test]
fn a_snapshot_remembers_that_the_board_was_discarded() {
    let mut board = a_board_holding(&a_idea("idea_1"), Spot::ORIGIN);
    board.apply(&BoardEvent::Discarded, &a_metadata(9));

    let snapshot = board.snapshot();

    assert_eq!(
        Board::from_first(&snapshot, &a_metadata(10)).expect("a snapshot should raise a board"),
        board,
        "compaction must not resurrect a discarded board by forgetting it ever ended"
    );
}
