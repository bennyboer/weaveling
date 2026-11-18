use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::AllowingEverything;
use axum::Extension;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{Agent, AggregateContext};
use inbox::{InMemoryInbox, Inboxed};
use live::testing::Subscriber;
use live::{Broadcaster, Topics};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::context::PROJECT;
use readings_catalog::{InMemoryOutlineCatalog, InMemorySceneTextCatalog};
use readings_core::{
    Block, OutlineCatalog, ReadingsCore, Run, SceneLink, SceneTextCatalog, SectionLink, ToldSection,
};
use readings_messaging::KeepSceneTextsListener;
use readings_rest::TellingFollowers;
use scenes_contract::{BlockDTO, MarksDTO, RunDTO, SceneTextStateDTO, TEXT_STATE_CHANGED};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

const THE_LOOM: &str = "project_loom";

struct Followed {
    readings: ReadingsCore,
    texts: Inboxed<KeepSceneTextsListener>,
    reader: Subscriber,
    _server: TestServer,
}

/// The core telling whoever follows a book, and a reader following the loom's.
async fn a_book_followed() -> Followed {
    let outlines: Arc<dyn OutlineCatalog> = Arc::new(InMemoryOutlineCatalog::new());
    let texts: Arc<dyn SceneTextCatalog> = Arc::new(InMemorySceneTextCatalog::new());
    let broadcaster = Broadcaster::new();
    let readings = ReadingsCore::new(
        outlines.clone(),
        texts.clone(),
        Arc::new(TellingFollowers::new(outlines, texts, broadcaster.clone())),
        Arc::new(InMemoryTransactions::new()),
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    );
    let server = TestServer::builder().http_transport().build(
        live::live_socket(Topics::of(vec![readings_rest::topic(
            readings.clone(),
            broadcaster,
        )]))
        .layer(Extension(Agent::Local)),
    );
    let mut reader = Subscriber::on(&server).await;
    reader
        .subscribe("book", "readings", &[("project", THE_LOOM)])
        .await;
    reader.following("book").await;

    Followed {
        texts: Inboxed::new(
            KeepSceneTextsListener::new(readings.clone()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
                1_700_000_000,
            ))))),
        ),
        readings,
        reader,
        _server: server,
    }
}

/// A scene's text as the scenes feature tells it, naming the scene's project.
fn settled(project: &str, scene: &str, words: &str, version: u64) -> Message {
    let mut payload = serde_json::to_value(SceneTextStateDTO {
        scene: scene.to_owned(),
        blocks: vec![BlockDTO::Paragraph {
            runs: vec![RunDTO::Text {
                text: words.to_owned(),
                marks: MarksDTO::default(),
            }],
        }],
        version,
    })
    .expect("a text is plain data");
    payload["context"] = AggregateContext::default().with(PROJECT, project).written();

    Message::opening(
        RoutingKey::parse(TEXT_STATE_CHANGED).expect("a declared key"),
        payload,
        clock::at(1_700_000_000),
    )
}

impl Followed {
    async fn hears(&self, message: Message) {
        self.texts
            .handle(&message)
            .await
            .expect("the listener should take this message");
    }

    async fn told(&mut self) -> Value {
        self.reader.frame().await["event"].clone()
    }
}

fn saying(words: &str) -> Value {
    json!([{ "kind": "paragraph", "runs": [
        { "kind": "text", "text": words, "marks": { "emphasis": false, "strong": false, "code": false, "link": null } }
    ] }])
}

#[tokio::test]
async fn a_scene_s_text_kept_is_told_to_whoever_follows_its_book() {
    // given: a reader following the book
    let mut followed = a_book_followed().await;

    // when: a scene of the book is told its text
    followed
        .hears(settled(THE_LOOM, "scene_harbour", "The boats came in.", 1))
        .await;

    // then: the reader is told the scene's text
    assert_eq!(
        followed.told().await,
        json!({ "kind": "text", "scene": "scene_harbour", "blocks": saying("The boats came in.") })
    );
}

#[tokio::test]
async fn the_book_s_sections_and_title_kept_are_told_to_whoever_follows_it() {
    // given: a reader following the book
    let mut followed = a_book_followed().await;

    // when: its sections are kept, a part under a chapter, and then its title
    followed
        .readings
        .outline_changed(
            None,
            THE_LOOM,
            &[
                ToldSection {
                    section: SectionLink::from("section_morning"),
                    parent: None,
                    title: "Morning".to_owned(),
                    scenes: vec![SceneLink::from("scene_harbour")],
                },
                ToldSection {
                    section: SectionLink::from("section_market"),
                    parent: Some(SectionLink::from("section_morning")),
                    title: "The market".to_owned(),
                    scenes: vec![],
                },
            ],
            3,
        )
        .await
        .expect("keeping the outline should succeed");
    followed
        .readings
        .title_changed(None, THE_LOOM, "The Loom", 1)
        .await
        .expect("keeping the title should succeed");

    // then: the reader is told the sections, each at its depth, and then the title
    assert_eq!(
        followed.told().await,
        json!({ "kind": "outline", "sections": [
            { "section": "section_morning", "depth": 0, "title": "Morning", "scenes": ["scene_harbour"] },
            { "section": "section_market", "depth": 1, "title": "The market", "scenes": [] },
        ] })
    );
    assert_eq!(
        followed.told().await,
        json!({ "kind": "title", "title": "The Loom" })
    );
}

#[tokio::test]
async fn a_late_text_and_another_book_s_text_are_not_told() {
    // given: a reader following the book, told a scene's text
    let mut followed = a_book_followed().await;
    followed
        .hears(settled(
            THE_LOOM,
            "scene_harbour",
            "The boats came in at dawn.",
            4,
        ))
        .await;
    followed.told().await;

    // when: an older text of it arrives late, and a scene of another book is told its text
    followed
        .hears(settled(THE_LOOM, "scene_harbour", "The boats came in.", 3))
        .await;
    followed
        .hears(settled("project_other", "scene_elsewhere", "Far away.", 1))
        .await;

    // then: the reader is told nothing more
    followed.reader.nothing().await;
}

#[tokio::test]
async fn a_text_whose_project_is_not_named_is_kept_but_told_to_nobody() {
    // given: a reader following the book
    let mut followed = a_book_followed().await;

    // when: a scene's text is kept without its project
    followed
        .readings
        .text_changed(
            None,
            None,
            "scene_harbour",
            &[Block::Paragraph(vec![Run::plain("The boats came in.")])],
            1,
        )
        .await
        .expect("keeping the text should succeed");

    // then: the reader is told nothing
    followed.reader.nothing().await;
}
