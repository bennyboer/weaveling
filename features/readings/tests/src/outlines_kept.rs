use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::{Agent, InMemoryEventStore};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use messaging::{Listener, Message, RoutingKey};
use outline_contract::{AttachmentDTO, OutlineStateDTO, STATE_CHANGED, SectionStateDTO};
use projects_contract::{DELETED, RENAMED, STARTED};
use readings_catalog::{InMemoryAccountNameCatalog, InMemoryReadingCopyCatalog};
use readings_catalog::{InMemoryOutlineCatalog, InMemorySceneTextCatalog};
use readings_core::{
    Block, Contents, NobodyFollows, Part, ProjectLink, ReadingsCore, ReadingsError, Run, SceneLink,
    SectionLink,
};
use readings_messaging::{KeepBookTitlesListener, KeepOutlinesListener};
use serde_json::json;
use transacting::InMemoryTransactions;

const THE_LOOM: &str = "project_loom";

struct Wired {
    readings: ReadingsCore,
    outlines: Inboxed<KeepOutlinesListener>,
    titles: Inboxed<KeepBookTitlesListener>,
}

fn inboxed<H: KeptHandler>(handler: H) -> Inboxed<H> {
    Inboxed::new(
        handler,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_700_000_000,
        ))))),
    )
}

/// The core keeping its books' outlines and titles as it hears them, where every scene has a text.
async fn a_workbench() -> Wired {
    let readings = ReadingsCore::new(
        Arc::new(InMemoryOutlineCatalog::new()),
        Arc::new(InMemorySceneTextCatalog::new()),
        Arc::new(InMemoryReadingCopyCatalog::new()),
        Arc::new(InMemoryAccountNameCatalog::new()),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(NobodyFollows),
        Arc::new(InMemoryTransactions::new()),
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    );
    for scene in ["scene_harbour", "scene_wall", "scene_stalls"] {
        readings
            .text_changed(None, None, scene, &saying(scene), 1)
            .await
            .expect("keeping a text should succeed");
    }

    Wired {
        outlines: inboxed(KeepOutlinesListener::new(readings.clone())),
        titles: inboxed(KeepBookTitlesListener::new(readings.clone())),
        readings,
    }
}

impl Wired {
    async fn hears(&self, message: Message) {
        let taken = if self.outlines.hears(&message.routing) {
            self.outlines.handle(&message).await
        } else {
            self.titles.handle(&message).await
        };
        taken.expect("the listener should take this message");
    }

    async fn book(&self) -> Result<Contents, ReadingsError> {
        self.readings.live_book(THE_LOOM, &Agent::Local).await
    }

    async fn read(&self) -> Contents {
        self.book().await.expect("the book should be put together")
    }
}

fn saying(words: &str) -> Vec<Block> {
    vec![Block::Paragraph(vec![Run::plain(words)])]
}

fn a_section(
    section: &str,
    parent: Option<&str>,
    attachments: &[AttachmentDTO],
) -> SectionStateDTO {
    SectionStateDTO {
        section: section.to_owned(),
        parent: parent.map(str::to_owned),
        title: format!("Title of {section}"),
        attachments: attachments.to_vec(),
    }
}

fn scene(id: &str) -> AttachmentDTO {
    AttachmentDTO::Scene { id: id.to_owned() }
}

fn idea(id: &str) -> AttachmentDTO {
    AttachmentDTO::Idea { id: id.to_owned() }
}

fn outline_at(version: u64, sections: Vec<SectionStateDTO>, discarded: bool) -> Message {
    Message::opening(
        RoutingKey::parse(STATE_CHANGED).expect("a declared key"),
        serde_json::to_value(OutlineStateDTO {
            outline: "outline_loom".to_owned(),
            project: THE_LOOM.to_owned(),
            version,
            discarded,
            sections,
        })
        .expect("an outline is plain data"),
        clock::at(1_700_000_000),
    )
}

fn morning_and_market(version: u64) -> Message {
    outline_at(
        version,
        vec![
            a_section(
                "section_morning",
                None,
                &[
                    scene("scene_harbour"),
                    idea("idea_tide"),
                    scene("scene_wall"),
                ],
            ),
            a_section(
                "section_market",
                Some("section_morning"),
                &[scene("scene_stalls")],
            ),
        ],
        false,
    )
}

fn market_alone(version: u64) -> Message {
    outline_at(
        version,
        vec![a_section("section_market", None, &[scene("scene_stalls")])],
        false,
    )
}

fn project_told(routing: &str, version: u64, event: serde_json::Value) -> Message {
    eventpublishing::published(
        routing,
        "project",
        THE_LOOM,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn started(name: &str) -> Message {
    project_told(
        STARTED,
        1,
        json!({ "version": 0, "name": "STARTED", "project_name": name }),
    )
}

fn renamed(version: u64, name: &str) -> Message {
    project_told(
        RENAMED,
        version,
        json!({ "version": 0, "name": "RENAMED", "project_name": name }),
    )
}

fn deleted(version: u64) -> Message {
    project_told(DELETED, version, json!({ "version": 0, "name": "DELETED" }))
}

fn sections_of(book: &Contents) -> Vec<(String, u8)> {
    book.parts
        .iter()
        .filter_map(|part| match part {
            Part::Section { section, depth, .. } => Some((section.to_string(), *depth)),
            Part::Scene { .. } => None,
        })
        .collect()
}

#[tokio::test]
async fn the_listeners_hear_the_outline_s_state_and_the_project_s_name_and_deletion() {
    // given: the listeners
    let wired = a_workbench().await;

    // expect: the outline's state, and the project's start, renaming and deletion
    assert!(
        wired
            .outlines
            .hears(&RoutingKey::parse(STATE_CHANGED).expect("a declared key"))
    );
    for routing in [STARTED, RENAMED, DELETED] {
        assert!(
            wired
                .titles
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn a_book_is_titled_by_its_project_and_laid_out_as_its_outline_tells_it() {
    // given: the listeners
    let wired = a_workbench().await;

    // when: the project's start and its outline are heard
    wired.hears(started("The Loom")).await;
    wired.hears(morning_and_market(5)).await;

    // then: the book has the project's name, each section nested under its parent, and the scenes but not the idea
    assert_eq!(
        wired.read().await,
        Contents {
            project: ProjectLink::from(THE_LOOM),
            title: "The Loom".to_owned(),
            parts: vec![
                Part::Section {
                    section: SectionLink::from("section_morning"),
                    depth: 0,
                    title: "Title of section_morning".to_owned(),
                },
                Part::Scene {
                    scene: SceneLink::from("scene_harbour"),
                    blocks: saying("scene_harbour"),
                },
                Part::Scene {
                    scene: SceneLink::from("scene_wall"),
                    blocks: saying("scene_wall"),
                },
                Part::Section {
                    section: SectionLink::from("section_market"),
                    depth: 1,
                    title: "Title of section_market".to_owned(),
                },
                Part::Scene {
                    scene: SceneLink::from("scene_stalls"),
                    blocks: saying("scene_stalls"),
                },
            ],
        }
    );
}

#[tokio::test]
async fn a_late_outline_does_not_undo_a_newer_one() {
    // given: an outline heard at a version
    let wired = a_workbench().await;
    wired.hears(market_alone(6)).await;

    // when: one told earlier arrives late
    wired.hears(morning_and_market(5)).await;

    // then: the newer one stands
    assert_eq!(
        sections_of(&wired.read().await),
        vec![("section_market".to_owned(), 0)]
    );
}

#[tokio::test]
async fn a_renamed_project_retitles_its_book_and_a_late_name_does_not_undo_it() {
    // given: a project started and renamed
    let wired = a_workbench().await;
    wired.hears(started("The Loom")).await;
    wired.hears(renamed(3, "The Loom, Again")).await;

    // when: an earlier name arrives late
    wired.hears(renamed(2, "Working Title")).await;

    // then: the book has the newest name
    assert_eq!(wired.read().await.title, "The Loom, Again");
}

#[tokio::test]
async fn a_deleted_project_s_book_is_gone_and_nothing_late_brings_it_back() {
    // given: a book heard of
    let wired = a_workbench().await;
    wired.hears(started("The Loom")).await;
    wired.hears(morning_and_market(5)).await;

    // when: its project is deleted, and a newer outline and name arrive late
    wired.hears(deleted(9)).await;
    wired.hears(market_alone(6)).await;
    wired.hears(renamed(8, "Too late")).await;

    // then: there is no book
    assert!(matches!(
        wired.book().await,
        Err(ReadingsError::ProjectNotThere)
    ));
}

#[tokio::test]
async fn a_discarded_outline_takes_its_book_with_it() {
    // given: a book heard of
    let wired = a_workbench().await;
    wired.hears(started("The Loom")).await;
    wired.hears(morning_and_market(5)).await;

    // when: its outline is told discarded
    wired.hears(outline_at(6, vec![], true)).await;

    // then: there is no book
    assert!(matches!(
        wired.book().await,
        Err(ReadingsError::ProjectNotThere)
    ));
}

#[tokio::test]
async fn a_project_nobody_told_of_has_no_book_and_one_without_an_outline_an_empty_one() {
    // given: the listeners
    let wired = a_workbench().await;

    // expect: no book before anything is heard
    assert!(matches!(
        wired.book().await,
        Err(ReadingsError::ProjectNotThere)
    ));

    // when: only the project's start is heard
    wired.hears(started("The Loom")).await;

    // then: its book has its title and nothing in it
    let book = wired.read().await;
    assert_eq!(book.title, "The Loom");
    assert!(book.parts.is_empty());
}
