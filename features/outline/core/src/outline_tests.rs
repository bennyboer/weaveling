use std::cell::RefCell;
use std::collections::HashMap;

use eventsourcing::{Agent, AgentId, Aggregate, AggregateId, EventMetadata, Version};
use time::{Duration, OffsetDateTime};

use crate::id::SectionId;
use crate::outline::{
    Attachment, KIND, Outline, OutlineCommand, OutlineError, OutlineEvent, PassageLink,
};
use crate::title::SectionTitle;

thread_local! {
    static MINTED: RefCell<HashMap<String, SectionId>> = RefCell::new(HashMap::new());
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_metadata() -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("outline_1"),
        kind: KIND,
        version: Version::of(1),
        agent: an_author(),
        occurred_at: at(1_000),
        is_snapshot: false,
    }
}

fn a_section(named: &str) -> SectionId {
    MINTED.with(|minted| {
        let mut minted = minted.borrow_mut();
        let next = minted.len() as i64;

        *minted
            .entry(named.to_owned())
            .or_insert_with(|| SectionId::generate(at(next)))
    })
}

fn a_passage(named: &str) -> Attachment {
    Attachment::passage(&format!("passage_{named}"))
}

fn prose(named: &str) -> PassageLink {
    PassageLink::from(format!("passage_{named}"))
}

fn an_idea(named: &str) -> Attachment {
    Attachment::idea(&format!("idea_{named}"))
}

fn titled(what: &str) -> SectionTitle {
    SectionTitle::new(what).expect("the title should be usable")
}

struct Book {
    outline: Outline,
}

impl Book {
    fn started() -> Self {
        MINTED.with(|minted| minted.borrow_mut().clear());

        let happened = Outline::begin(
            OutlineCommand::Start {
                project: "project_1".into(),
            },
            &an_author(),
        )
        .expect("an outline should start");
        let outline = Outline::from_first(&happened[0], &a_metadata())
            .expect("the first event should raise an outline");

        Self { outline }
    }

    fn does(&mut self, command: OutlineCommand) -> Vec<OutlineEvent> {
        let happened = self
            .outline
            .decide(command, &an_author())
            .expect("the command should be accepted");

        for event in &happened {
            self.outline.apply(event, &a_metadata());
        }

        happened
    }

    fn refuses(&self, command: OutlineCommand) -> OutlineError {
        self.outline
            .decide(command, &an_author())
            .expect_err("the command should be refused")
    }

    fn adds(&mut self, named: &str, under: Option<&str>, after: Option<&str>) {
        self.does(OutlineCommand::Add {
            section: a_section(named),
            under: under.map(a_section),
            after: after.map(a_section),
            title: titled(named),
        });
    }

    fn attaches_passage(&mut self, passage: &str, to: &str) {
        self.attaches(a_passage(passage), to);
    }

    fn attaches_idea(&mut self, idea: &str, to: &str) {
        self.attaches(an_idea(idea), to);
    }

    fn attaches(&mut self, attachment: Attachment, to: &str) {
        let held = self.outline.attachments_in(&a_section(to));

        self.does(OutlineCommand::Attach {
            attachment,
            to: a_section(to),
            after: held.last().cloned(),
        });
    }

    fn shape(&self) -> String {
        self.outline
            .sections()
            .into_iter()
            .map(|placed| {
                let mut depth = 0;
                let mut walking = placed.parent;

                while let Some(here) = walking {
                    depth += 1;
                    walking = self.outline.parent_of(&here);
                }

                format!("{}{}", "  ".repeat(depth), placed.title)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn shaped(drawn: &str) -> String {
    let lines: Vec<&str> = drawn
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let flush = lines
        .iter()
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or_default();

    lines
        .iter()
        .map(|line| &line[flush..])
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_outline_starts_empty_because_a_book_has_no_mandatory_root() {
    let book = Book::started();

    assert!(book.outline.children().is_empty());
    assert!(book.outline.sections().is_empty());
}

#[test]
fn sections_are_added_in_the_order_they_are_asked_for() {
    let mut book = Book::started();

    book.adds("One", None, None);
    book.adds("Two", None, Some("One"));
    book.adds("Three", None, Some("Two"));

    assert_eq!(
        book.shape(),
        shaped(
            "
            One
            Two
            Three
            "
        )
    );
}

#[test]
fn a_section_added_after_nothing_goes_to_the_front() {
    let mut book = Book::started();

    book.adds("Two", None, None);
    book.adds("One", None, None);

    assert_eq!(
        book.shape(),
        shaped(
            "
            One
            Two
            "
        )
    );
}

#[test]
fn a_section_nests_under_the_one_it_was_added_to() {
    let mut book = Book::started();

    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));

    assert_eq!(
        book.shape(),
        shaped(
            "
            Part One
              Chapter 1
              Chapter 2
            "
        )
    );
}

#[test]
fn a_section_cannot_be_added_twice() {
    let mut book = Book::started();
    book.adds("One", None, None);

    let refused = book.refuses(OutlineCommand::Add {
        section: a_section("One"),
        under: None,
        after: None,
        title: titled("One again"),
    });

    assert_eq!(refused, OutlineError::AlreadyThere);
}

#[test]
fn a_section_cannot_be_added_under_one_that_is_not_there() {
    let book = Book::started();

    let refused = book.refuses(OutlineCommand::Add {
        section: a_section("Orphan"),
        under: Some(a_section("Nowhere")),
        after: None,
        title: titled("Orphan"),
    });

    assert_eq!(refused, OutlineError::NoSuchSection);
}

#[test]
fn retitling_to_the_same_words_is_not_worth_an_event() {
    let mut book = Book::started();
    book.adds("One", None, None);

    let happened = book.does(OutlineCommand::Retitle {
        section: a_section("One"),
        title: titled("One"),
    });

    assert!(happened.is_empty());
}

#[test]
fn a_section_may_go_unnamed_so_that_it_can_borrow_the_title_of_its_passage() {
    let mut book = Book::started();
    book.adds("One", None, None);

    book.does(OutlineCommand::Retitle {
        section: a_section("One"),
        title: SectionTitle::unnamed(),
    });

    assert!(
        book.outline
            .title_of(&a_section("One"))
            .expect("the section should still be there")
            .is_unnamed()
    );
}

#[test]
fn promoting_lifts_a_section_out_to_sit_after_its_old_parent() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Rain", Some("Chapter 1"), Some("Arrival"));
    book.adds("Chapter 2", None, Some("Chapter 1"));

    book.does(OutlineCommand::Promote {
        section: a_section("Rain"),
    });

    assert_eq!(
        book.shape(),
        shaped(
            "
            Chapter 1
              Arrival
            Rain
            Chapter 2
            "
        )
    );
}

#[test]
fn promoting_takes_the_sections_that_followed_it_along_as_children() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Rain", Some("Chapter 1"), Some("Arrival"));
    book.adds("Storm", Some("Chapter 1"), Some("Rain"));
    book.adds("Chapter 2", None, Some("Chapter 1"));

    book.does(OutlineCommand::Promote {
        section: a_section("Rain"),
    });

    assert_eq!(
        book.shape(),
        shaped(
            "
            Chapter 1
              Arrival
            Rain
              Storm
            Chapter 2
            "
        )
    );
}

#[test]
fn promoting_never_changes_what_the_book_reads_like() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Rain", Some("Chapter 1"), Some("Arrival"));
    book.adds("Storm", Some("Chapter 1"), Some("Rain"));
    book.attaches_passage("arrival", "Arrival");
    book.attaches_passage("rain", "Rain");
    book.attaches_passage("storm", "Storm");
    let before = book.outline.reading_order();

    book.does(OutlineCommand::Promote {
        section: a_section("Rain"),
    });

    assert_eq!(book.outline.reading_order(), before);
}

#[test]
fn a_section_at_the_top_has_nothing_to_be_promoted_out_of() {
    let mut book = Book::started();
    book.adds("One", None, None);

    let happened = book.does(OutlineCommand::Promote {
        section: a_section("One"),
    });

    assert!(happened.is_empty());
    assert_eq!(
        book.shape(),
        shaped(
            "
            One
            "
        )
    );
}

#[test]
fn demoting_nests_a_section_under_the_one_before_it() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Chapter 2", None, Some("Chapter 1"));

    book.does(OutlineCommand::Demote {
        section: a_section("Chapter 2"),
    });

    assert_eq!(
        book.shape(),
        shaped(
            "
            Chapter 1
              Chapter 2
            "
        )
    );
}

#[test]
fn demoting_puts_a_section_after_the_children_its_new_parent_already_had() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Chapter 2", None, Some("Chapter 1"));

    book.does(OutlineCommand::Demote {
        section: a_section("Chapter 2"),
    });

    assert_eq!(
        book.shape(),
        shaped(
            "
            Chapter 1
              Arrival
              Chapter 2
            "
        )
    );
}

#[test]
fn the_first_of_its_siblings_has_nothing_to_be_demoted_under() {
    let mut book = Book::started();
    book.adds("One", None, None);
    book.adds("Two", None, Some("One"));

    let happened = book.does(OutlineCommand::Demote {
        section: a_section("One"),
    });

    assert!(happened.is_empty());
    assert_eq!(
        book.shape(),
        shaped(
            "
            One
            Two
            "
        )
    );
}

#[test]
fn demoting_and_promoting_again_puts_a_section_back_where_it_was() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Chapter 2", None, Some("Chapter 1"));
    book.adds("Chapter 3", None, Some("Chapter 2"));
    let before = book.shape();

    book.does(OutlineCommand::Demote {
        section: a_section("Chapter 3"),
    });
    book.does(OutlineCommand::Promote {
        section: a_section("Chapter 3"),
    });

    assert_eq!(book.shape(), before);
}

#[test]
fn a_section_cannot_be_moved_inside_itself() {
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);

    let refused = book.refuses(OutlineCommand::Move {
        section: a_section("Part One"),
        under: Some(a_section("Chapter 1")),
        after: None,
    });

    assert_eq!(refused, OutlineError::WouldContainItself);
}

#[test]
fn a_move_carries_the_whole_subtree_with_it() {
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Part Two", None, Some("Part One"));
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Arrival", Some("Chapter 1"), None);

    book.does(OutlineCommand::Move {
        section: a_section("Chapter 1"),
        under: Some(a_section("Part Two")),
        after: None,
    });

    assert_eq!(
        book.shape(),
        shaped(
            "
            Part One
            Part Two
              Chapter 1
                Arrival
            "
        )
    );
}

#[test]
fn a_move_that_names_a_neighbour_which_is_not_there_is_refused() {
    let mut book = Book::started();
    book.adds("One", None, None);
    book.adds("Two", None, Some("One"));
    book.adds("Chapter", Some("One"), None);

    let refused = book.refuses(OutlineCommand::Move {
        section: a_section("Two"),
        under: None,
        after: Some(a_section("Chapter")),
    });

    assert_eq!(refused, OutlineError::NoSuchNeighbour);
}

#[test]
fn removing_a_section_lifts_its_children_into_its_place() {
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));
    book.adds("Part Two", None, Some("Part One"));

    book.does(OutlineCommand::Remove {
        section: a_section("Part One"),
    });

    assert_eq!(
        book.shape(),
        shaped(
            "
            Chapter 1
            Chapter 2
            Part Two
            "
        )
    );
}

#[test]
fn removing_a_section_returns_its_passages_to_the_pool_rather_than_losing_them() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_passage("arrival", "Chapter 1");

    book.does(OutlineCommand::Remove {
        section: a_section("Chapter 1"),
    });

    assert!(book.outline.reading_order().is_empty());
    assert_eq!(book.outline.section_holding(&a_passage("arrival")), None);
}

#[test]
fn a_passage_attached_where_it_already_sits_elsewhere_simply_moves() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Chapter 2", None, Some("Chapter 1"));
    book.attaches_passage("rain", "Chapter 1");

    book.attaches_passage("rain", "Chapter 2");

    assert!(
        book.outline
            .attachments_in(&a_section("Chapter 1"))
            .is_empty()
    );
    assert_eq!(
        book.outline.section_holding(&a_passage("rain")),
        Some(a_section("Chapter 2"))
    );
}

#[test]
fn several_passages_may_sit_in_one_section_in_the_order_they_are_read() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    book.attaches_passage("one", "Chapter 1");
    book.attaches_passage("two", "Chapter 1");
    book.attaches_passage("three", "Chapter 1");

    assert_eq!(
        book.outline.reading_order(),
        vec![prose("one"), prose("two"), prose("three")]
    );
}

#[test]
fn the_reading_order_walks_the_whole_book_depth_first() {
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));
    book.adds("Part Two", None, Some("Part One"));
    book.attaches_passage("epigraph", "Part One");
    book.attaches_passage("arrival", "Chapter 1");
    book.attaches_passage("rain", "Chapter 2");
    book.attaches_passage("after", "Part Two");

    assert_eq!(
        book.outline.reading_order(),
        vec![
            prose("epigraph"),
            prose("arrival"),
            prose("rain"),
            prose("after")
        ]
    );
}

#[test]
fn a_passage_that_is_not_in_the_outline_cannot_be_detached() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    let refused = book.refuses(OutlineCommand::Detach {
        attachment: a_passage("nowhere"),
    });

    assert_eq!(refused, OutlineError::NotAttached);
}

#[test]
fn a_snapshot_rebuilds_the_same_book() {
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));
    book.adds("Part Two", None, Some("Part One"));
    book.attaches_passage("arrival", "Arrival");
    book.attaches_passage("rain", "Chapter 2");

    let rebuilt = Outline::from_first(&book.outline.snapshot(), &a_metadata())
        .expect("a snapshot should raise an outline");

    assert_eq!(rebuilt.sections(), book.outline.sections());
    assert_eq!(rebuilt.reading_order(), book.outline.reading_order());
}

#[test]
fn an_outline_can_be_discarded() {
    let mut book = Book::started();

    let happened = book.does(OutlineCommand::Discard);

    assert_eq!(happened, vec![OutlineEvent::Discarded]);
    assert!(book.outline.is_discarded());
}

#[test]
fn a_discarded_outline_refuses_everything() {
    let mut book = Book::started();
    let chapter = a_section("one");
    book.does(OutlineCommand::Add {
        section: chapter,
        under: None,
        after: None,
        title: titled("Chapter One"),
    });
    book.does(OutlineCommand::Discard);

    for command in [
        OutlineCommand::Start {
            project: "project_1".into(),
        },
        OutlineCommand::Add {
            section: a_section("two"),
            under: None,
            after: None,
            title: titled("Chapter Two"),
        },
        OutlineCommand::Retitle {
            section: chapter,
            title: titled("Renamed"),
        },
        OutlineCommand::Remove { section: chapter },
        OutlineCommand::Discard,
    ] {
        assert_eq!(
            book.outline.decide(command, &an_author()),
            Err(OutlineError::Discarded),
            "an outline is discarded when its project is deleted, so anything still holding a \
             reference to it must be refused rather than quietly allowed"
        );
    }
}

#[test]
fn a_snapshot_remembers_that_the_outline_was_discarded() {
    let mut book = Book::started();
    book.does(OutlineCommand::Add {
        section: a_section("one"),
        under: None,
        after: None,
        title: titled("Chapter One"),
    });
    book.does(OutlineCommand::Discard);

    let snapshot = book.outline.snapshot();

    assert_eq!(
        Outline::from_first(&snapshot, &a_metadata()).expect("a snapshot should raise an outline"),
        book.outline,
        "compaction must not resurrect a discarded outline by forgetting it ever ended"
    );
}

#[test]
fn an_idea_sits_in_a_section_as_a_note_beside_the_prose() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    book.attaches_passage("arrival", "Chapter 1");
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");

    assert_eq!(
        book.outline.attachments_in(&a_section("Chapter 1")),
        vec![a_passage("arrival"), an_idea("foreshadow-the-loom")]
    );
}

#[test]
fn a_note_is_not_part_of_the_manuscript() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");
    book.attaches_passage("arrival", "Chapter 1");

    assert_eq!(
        book.outline.reading_order(),
        vec![prose("arrival")],
        "an idea pinned to a chapter is a note to the author, so exporting the book must \
         walk straight past it"
    );
}

#[test]
fn an_idea_and_a_passage_sharing_a_name_are_two_different_attachments() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    book.attaches(Attachment::passage("same_1"), "Chapter 1");
    book.attaches(Attachment::idea("same_1"), "Chapter 1");

    assert_eq!(
        book.outline.attachments_in(&a_section("Chapter 1")).len(),
        2,
        "the tag is part of the identity, or attaching an idea would silently move the \
         passage whose id happens to match"
    );
}

#[test]
fn a_note_can_be_detached_without_touching_the_prose() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_passage("arrival", "Chapter 1");
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");

    book.does(OutlineCommand::Detach {
        attachment: an_idea("foreshadow-the-loom"),
    });

    assert_eq!(
        book.outline.attachments_in(&a_section("Chapter 1")),
        vec![a_passage("arrival")]
    );
}

#[test]
fn the_index_is_told_about_notes_as_well_as_prose() {
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_passage("arrival", "Chapter 1");
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");

    assert_eq!(
        book.outline.attachments(),
        vec![a_passage("arrival"), an_idea("foreshadow-the-loom")],
        "the index is what lets a discarded idea find the books holding it, so feeding it \
         the reading order would leave every note unreachable"
    );
}
