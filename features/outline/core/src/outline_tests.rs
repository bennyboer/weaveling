use projects_contract::context::PROJECT;
use std::cell::RefCell;
use std::collections::HashMap;

use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, EventMetadata, Version,
};
use time::{Duration, OffsetDateTime};

use crate::id::SectionId;
use crate::outline::{
    Attachment, KIND, Outline, OutlineCommand, OutlineError, OutlineEvent, SceneLink,
};
use crate::title::SectionTitle;

thread_local! {
    static MINTED: RefCell<HashMap<String, SectionId>> = RefCell::new(HashMap::new());
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::Account(AccountId::from("author-7"))
}

fn a_metadata() -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("outline_1"),
        kind: KIND,
        version: Version::of(1),
        agent: an_author(),
        occurred_at: at(1_000),
        is_snapshot: false,
        context: AggregateContext::default(),
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

fn a_scene(named: &str) -> Attachment {
    Attachment::scene(&format!("scene_{named}"))
}

fn a_scene_link(named: &str) -> SceneLink {
    SceneLink::from(format!("scene_{named}"))
}

fn an_idea(named: &str) -> Attachment {
    Attachment::idea(&format!("idea_{named}"))
}

fn a_title(what: &str) -> SectionTitle {
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
            title: a_title(named),
        });
    }

    fn attaches_scene(&mut self, scene: &str, to: &str) {
        self.attaches(a_scene(scene), to);
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
    // when: an outline is started
    let book = Book::started();

    // then: it holds no sections
    assert!(book.outline.children().is_empty());
    assert!(book.outline.sections().is_empty());
}

#[test]
fn sections_are_added_in_the_order_they_are_asked_for() {
    // given: a started outline
    let mut book = Book::started();

    // when: sections are added, each after the one before
    book.adds("One", None, None);
    book.adds("Two", None, Some("One"));
    book.adds("Three", None, Some("Two"));

    // then: they stand in that order
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
    // given: a started outline
    let mut book = Book::started();

    // when: sections are added after nothing
    book.adds("Two", None, None);
    book.adds("One", None, None);

    // then: the last added stands first
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
    // given: a started outline
    let mut book = Book::started();

    // when: sections are added under another
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));

    // then: they nest under it
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
    // given: an outline with a section
    let mut book = Book::started();
    book.adds("One", None, None);

    // when: the section is added again
    let refused = book.refuses(OutlineCommand::Add {
        section: a_section("One"),
        under: None,
        after: None,
        title: a_title("One again"),
    });

    // then: it is refused as already there
    assert_eq!(refused, OutlineError::AlreadyThere);
}

#[test]
fn a_section_cannot_be_added_under_one_that_is_not_there() {
    // given: a started outline
    let book = Book::started();

    // when: a section is added under one that is not there
    let refused = book.refuses(OutlineCommand::Add {
        section: a_section("Orphan"),
        under: Some(a_section("Nowhere")),
        after: None,
        title: a_title("Orphan"),
    });

    // then: it is refused as no such section
    assert_eq!(refused, OutlineError::NoSuchSection);
}

#[test]
fn retitling_to_the_same_words_is_not_worth_an_event() {
    // given: an outline with a section
    let mut book = Book::started();
    book.adds("One", None, None);

    // when: it is retitled to the title it has
    let happened = book.does(OutlineCommand::Retitle {
        section: a_section("One"),
        title: a_title("One"),
    });

    // then: nothing happens
    assert!(happened.is_empty());
}

#[test]
fn a_section_may_be_left_unnamed_again() {
    // given: an outline with a named section
    let mut book = Book::started();
    book.adds("One", None, None);

    // when: it is left unnamed
    book.does(OutlineCommand::Retitle {
        section: a_section("One"),
        title: SectionTitle::unnamed(),
    });

    // then: it is unnamed
    assert!(
        book.outline
            .title_of(&a_section("One"))
            .expect("the section should still be there")
            .is_unnamed()
    );
}

#[test]
fn promoting_lifts_a_section_out_to_sit_after_its_old_parent() {
    // given: a chapter with two sections in it, and a chapter after it
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Rain", Some("Chapter 1"), Some("Arrival"));
    book.adds("Chapter 2", None, Some("Chapter 1"));

    // when: the second section is promoted
    book.does(OutlineCommand::Promote {
        section: a_section("Rain"),
    });

    // then: it sits after its old chapter
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
    // given: a chapter with three sections in it, and a chapter after it
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Rain", Some("Chapter 1"), Some("Arrival"));
    book.adds("Storm", Some("Chapter 1"), Some("Rain"));
    book.adds("Chapter 2", None, Some("Chapter 1"));

    // when: the middle section is promoted
    book.does(OutlineCommand::Promote {
        section: a_section("Rain"),
    });

    // then: the section after it goes along as its child
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
    // given: a chapter with three sections in it, each holding a scene
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Rain", Some("Chapter 1"), Some("Arrival"));
    book.adds("Storm", Some("Chapter 1"), Some("Rain"));
    book.attaches_scene("arrival", "Arrival");
    book.attaches_scene("rain", "Rain");
    book.attaches_scene("storm", "Storm");
    // and: the order the book reads in
    let before = book.outline.reading_order();

    // when: the middle section is promoted
    book.does(OutlineCommand::Promote {
        section: a_section("Rain"),
    });

    // then: the book reads as before
    assert_eq!(book.outline.reading_order(), before);
}

#[test]
fn a_section_at_the_top_has_nothing_to_be_promoted_out_of() {
    // given: an outline with a section at the top
    let mut book = Book::started();
    book.adds("One", None, None);

    // when: the section is promoted
    let happened = book.does(OutlineCommand::Promote {
        section: a_section("One"),
    });

    // then: nothing happens
    assert!(happened.is_empty());
    // and: the outline stays as it was
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
    // given: two chapters
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Chapter 2", None, Some("Chapter 1"));

    // when: the second is demoted
    book.does(OutlineCommand::Demote {
        section: a_section("Chapter 2"),
    });

    // then: it nests under the first
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
    // given: a chapter with a section in it, and a chapter after it
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Chapter 2", None, Some("Chapter 1"));

    // when: the second chapter is demoted
    book.does(OutlineCommand::Demote {
        section: a_section("Chapter 2"),
    });

    // then: it nests after the section already there
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
    // given: two sections
    let mut book = Book::started();
    book.adds("One", None, None);
    book.adds("Two", None, Some("One"));

    // when: the first is demoted
    let happened = book.does(OutlineCommand::Demote {
        section: a_section("One"),
    });

    // then: nothing happens
    assert!(happened.is_empty());
    // and: the outline stays as it was
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
    // given: three chapters
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Chapter 2", None, Some("Chapter 1"));
    book.adds("Chapter 3", None, Some("Chapter 2"));
    // and: the shape they make
    let before = book.shape();

    // when: the last is demoted and promoted again
    book.does(OutlineCommand::Demote {
        section: a_section("Chapter 3"),
    });
    book.does(OutlineCommand::Promote {
        section: a_section("Chapter 3"),
    });

    // then: the shape is as before
    assert_eq!(book.shape(), before);
}

#[test]
fn a_section_cannot_be_moved_inside_itself() {
    // given: a part with a chapter in it
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);

    // when: the part is moved under the chapter
    let refused = book.refuses(OutlineCommand::Move {
        section: a_section("Part One"),
        under: Some(a_section("Chapter 1")),
        after: None,
    });

    // then: it is refused as it would contain itself
    assert_eq!(refused, OutlineError::WouldContainItself);
}

#[test]
fn a_move_carries_the_whole_subtree_with_it() {
    // given: two parts, the first with a chapter holding a section
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Part Two", None, Some("Part One"));
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Arrival", Some("Chapter 1"), None);

    // when: the chapter is moved under the second part
    book.does(OutlineCommand::Move {
        section: a_section("Chapter 1"),
        under: Some(a_section("Part Two")),
        after: None,
    });

    // then: its section goes along
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
    // given: two sections, the first with a chapter in it
    let mut book = Book::started();
    book.adds("One", None, None);
    book.adds("Two", None, Some("One"));
    book.adds("Chapter", Some("One"), None);

    // when: the second is moved after the chapter, but not under the first
    let refused = book.refuses(OutlineCommand::Move {
        section: a_section("Two"),
        under: None,
        after: Some(a_section("Chapter")),
    });

    // then: it is refused as no such neighbour
    assert_eq!(refused, OutlineError::NoSuchNeighbour);
}

#[test]
fn removing_a_section_lifts_its_children_into_its_place() {
    // given: a part with two chapters in it, and a part after it
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));
    book.adds("Part Two", None, Some("Part One"));

    // when: the first part is removed
    book.does(OutlineCommand::Remove {
        section: a_section("Part One"),
    });

    // then: its chapters take its place
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
fn removing_a_section_returns_its_scenes_to_the_pool_rather_than_losing_them() {
    // given: a chapter holding a scene
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_scene("arrival", "Chapter 1");

    // when: the chapter is removed
    book.does(OutlineCommand::Remove {
        section: a_section("Chapter 1"),
    });

    // then: the book reads nothing
    assert!(book.outline.reading_order().is_empty());
    // and: the scene sits in no section
    assert_eq!(book.outline.section_holding(&a_scene("arrival")), None);
}

#[test]
fn a_scene_attached_where_it_already_sits_elsewhere_simply_moves() {
    // given: two chapters, the first holding a scene
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Chapter 2", None, Some("Chapter 1"));
    book.attaches_scene("rain", "Chapter 1");

    // when: the scene is attached to the second
    book.attaches_scene("rain", "Chapter 2");

    // then: the first holds nothing
    assert!(
        book.outline
            .attachments_in(&a_section("Chapter 1"))
            .is_empty()
    );
    // and: the scene sits in the second
    assert_eq!(
        book.outline.section_holding(&a_scene("rain")),
        Some(a_section("Chapter 2"))
    );
}

#[test]
fn several_scenes_may_sit_in_one_section_in_the_order_they_are_read() {
    // given: a chapter
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    // when: three scenes are attached to it one after another
    book.attaches_scene("one", "Chapter 1");
    book.attaches_scene("two", "Chapter 1");
    book.attaches_scene("three", "Chapter 1");

    // then: the book reads them in that order
    assert_eq!(
        book.outline.reading_order(),
        vec![
            a_scene_link("one"),
            a_scene_link("two"),
            a_scene_link("three")
        ]
    );
}

#[test]
fn the_reading_order_walks_the_whole_book_depth_first() {
    // given: parts and chapters, each holding a scene
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));
    book.adds("Part Two", None, Some("Part One"));
    book.attaches_scene("epigraph", "Part One");
    book.attaches_scene("arrival", "Chapter 1");
    book.attaches_scene("rain", "Chapter 2");
    book.attaches_scene("after", "Part Two");

    // then: the book reads them depth first
    assert_eq!(
        book.outline.reading_order(),
        vec![
            a_scene_link("epigraph"),
            a_scene_link("arrival"),
            a_scene_link("rain"),
            a_scene_link("after")
        ]
    );
}

#[test]
fn a_scene_that_is_not_in_the_outline_cannot_be_detached() {
    // given: an outline with a chapter
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    // when: a scene not in the outline is detached
    let refused = book.refuses(OutlineCommand::Detach {
        attachment: a_scene("nowhere"),
    });

    // then: it is refused as not attached
    assert_eq!(refused, OutlineError::NotAttached);
}

#[test]
fn a_snapshot_rebuilds_the_same_book() {
    // given: parts, chapters and sections, some holding scenes
    let mut book = Book::started();
    book.adds("Part One", None, None);
    book.adds("Chapter 1", Some("Part One"), None);
    book.adds("Arrival", Some("Chapter 1"), None);
    book.adds("Chapter 2", Some("Part One"), Some("Chapter 1"));
    book.adds("Part Two", None, Some("Part One"));
    book.attaches_scene("arrival", "Arrival");
    book.attaches_scene("rain", "Chapter 2");

    // when: an outline is raised from its snapshot
    let rebuilt = Outline::from_first(&book.outline.snapshot(), &a_metadata())
        .expect("a snapshot should raise an outline");

    // then: it has the same sections
    assert_eq!(rebuilt.sections(), book.outline.sections());
    // and: it reads the same
    assert_eq!(rebuilt.reading_order(), book.outline.reading_order());
}

#[test]
fn an_outline_can_be_discarded() {
    // given: a started outline
    let mut book = Book::started();

    // when: it is discarded
    let happened = book.does(OutlineCommand::Discard);

    // then: it says so
    assert_eq!(happened, vec![OutlineEvent::Discarded]);
    // and: it is discarded
    assert!(book.outline.is_discarded());
}

#[test]
fn a_discarded_outline_refuses_everything() {
    // given: a discarded outline with a chapter
    let mut book = Book::started();
    let chapter = a_section("one");
    book.does(OutlineCommand::Add {
        section: chapter,
        under: None,
        after: None,
        title: a_title("Chapter One"),
    });
    book.does(OutlineCommand::Discard);

    // then: every command is refused as discarded
    for command in [
        OutlineCommand::Start {
            project: "project_1".into(),
        },
        OutlineCommand::Add {
            section: a_section("two"),
            under: None,
            after: None,
            title: a_title("Chapter Two"),
        },
        OutlineCommand::Retitle {
            section: chapter,
            title: a_title("Renamed"),
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
    // given: a discarded outline with a chapter
    let mut book = Book::started();
    book.does(OutlineCommand::Add {
        section: a_section("one"),
        under: None,
        after: None,
        title: a_title("Chapter One"),
    });
    book.does(OutlineCommand::Discard);

    // when: it is snapshotted
    let snapshot = book.outline.snapshot();

    // then: the outline raised from the snapshot is discarded as well
    assert_eq!(
        Outline::from_first(&snapshot, &a_metadata()).expect("a snapshot should raise an outline"),
        book.outline,
        "compaction must not resurrect a discarded outline by forgetting it ever ended"
    );
}

#[test]
fn an_idea_sits_in_a_section_as_a_note_beside_the_scenes() {
    // given: a chapter
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    // when: a scene and an idea are attached to it
    book.attaches_scene("arrival", "Chapter 1");
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");

    // then: it holds both in that order
    assert_eq!(
        book.outline.attachments_in(&a_section("Chapter 1")),
        vec![a_scene("arrival"), an_idea("foreshadow-the-loom")]
    );
}

#[test]
fn a_note_is_not_part_of_the_manuscript() {
    // given: a chapter holding an idea and a scene
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");
    book.attaches_scene("arrival", "Chapter 1");

    // then: the book reads only the scene
    assert_eq!(
        book.outline.reading_order(),
        vec![a_scene_link("arrival")],
        "an idea pinned to a chapter is a note to the author, so exporting the book must \
         walk straight past it"
    );
}

#[test]
fn an_idea_and_a_scene_sharing_a_name_are_two_different_attachments() {
    // given: a chapter
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    // when: a scene and an idea of the same id are attached to it
    book.attaches(Attachment::scene("same_1"), "Chapter 1");
    book.attaches(Attachment::idea("same_1"), "Chapter 1");

    // then: it holds both
    assert_eq!(
        book.outline.attachments_in(&a_section("Chapter 1")).len(),
        2,
        "the tag is part of the identity, or attaching an idea would silently move the \
         scene whose id happens to match"
    );
}

#[test]
fn a_note_can_be_detached_without_touching_the_scenes() {
    // given: a chapter holding a scene and an idea
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_scene("arrival", "Chapter 1");
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");

    // when: the idea is detached
    book.does(OutlineCommand::Detach {
        attachment: an_idea("foreshadow-the-loom"),
    });

    // then: the chapter holds the scene alone
    assert_eq!(
        book.outline.attachments_in(&a_section("Chapter 1")),
        vec![a_scene("arrival")]
    );
}

#[test]
fn the_index_is_told_about_notes_as_well_as_scenes() {
    // given: a chapter holding a scene and an idea
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_scene("arrival", "Chapter 1");
    book.attaches_idea("foreshadow-the-loom", "Chapter 1");

    // then: both are among the outline's attachments
    assert_eq!(
        book.outline.attachments(),
        vec![a_scene("arrival"), an_idea("foreshadow-the-loom")],
        "the index is what lets a discarded idea find the books holding it, so feeding it \
         the reading order would leave every note unreachable"
    );
}

#[test]
fn a_first_attachment_is_announced_as_one_event() {
    // given: an empty chapter
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);

    // when: an idea is attached to it
    let happened = book.does(OutlineCommand::Attach {
        attachment: an_idea("loom"),
        to: a_section("Chapter 1"),
        after: None,
    });

    // then: the attaching is all that happens
    assert_eq!(
        happened,
        vec![OutlineEvent::Attached {
            attachment: an_idea("loom"),
            to: a_section("Chapter 1"),
            after: None,
        }]
    );
}

#[test]
fn moving_an_attachment_says_it_left_before_it_says_where_it_went() {
    // given: two chapters, the first holding an idea
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.adds("Chapter 2", None, Some("Chapter 1"));
    book.attaches_idea("loom", "Chapter 1");

    // when: the idea is attached to the second
    let happened = book.does(OutlineCommand::Attach {
        attachment: an_idea("loom"),
        to: a_section("Chapter 2"),
        after: None,
    });

    // then: it leaves the first before it is attached to the second
    assert_eq!(
        happened,
        vec![
            OutlineEvent::Detached {
                attachment: an_idea("loom"),
                from: a_section("Chapter 1"),
            },
            OutlineEvent::Attached {
                attachment: an_idea("loom"),
                to: a_section("Chapter 2"),
                after: None,
            },
        ],
        "an attachment names only where it goes, so a listener keeping its own record of where \
         things sit learns of the old place from the detach or not at all"
    );
    // and: the first holds nothing
    assert_eq!(
        book.outline.attachments_in(&a_section("Chapter 1")),
        Vec::new()
    );
}

#[test]
fn a_detachment_names_the_section_it_left() {
    // given: a chapter holding an idea
    let mut book = Book::started();
    book.adds("Chapter 1", None, None);
    book.attaches_idea("loom", "Chapter 1");

    // when: the idea is detached
    let happened = book.does(OutlineCommand::Detach {
        attachment: an_idea("loom"),
    });

    // then: the detaching names the chapter
    assert_eq!(
        happened,
        vec![OutlineEvent::Detached {
            attachment: an_idea("loom"),
            from: a_section("Chapter 1"),
        }],
        "a listener keyed by section cannot forget a note without being told which section"
    );
}

#[test]
fn an_outline_is_in_the_context_of_its_project() {
    // expect: an outline is in the context of its project
    assert_eq!(
        Book::started().outline.context().get(PROJECT),
        Some("project_1")
    );
}
