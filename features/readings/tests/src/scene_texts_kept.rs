use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::Agent;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use readings_catalog::{InMemorySceneTextCatalog, SampleBook};
use readings_core::{Block, Part, ReadingsCore, Run};
use readings_messaging::KeepSceneTextsListener;
use scenes_contract::{
    BlockDTO, DELETED, MarksDTO, RunDTO, SceneDeletedDTO, SceneTextSettledDTO, TEXT_SETTLED,
};
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

struct Wired {
    readings: ReadingsCore,
    keeping: Inboxed<KeepSceneTextsListener>,
}

/// The core around the sample outline, whose scenes' texts are kept as they are heard.
fn a_workbench() -> Wired {
    let readings = ReadingsCore::new(
        Arc::new(SampleBook),
        Arc::new(InMemorySceneTextCatalog::new()),
        Arc::new(InMemoryTransactions::new()),
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    );

    Wired {
        keeping: Inboxed::new(
            KeepSceneTextsListener::new(readings.clone()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                OffsetDateTime::UNIX_EPOCH,
            )))),
        ),
        readings,
    }
}

impl Wired {
    async fn hears(&self, message: Message) {
        self.keeping
            .handle(&message)
            .await
            .expect("the listener should take this message");
    }

    /// The scenes of the book, each with its text, in reading order.
    async fn scenes(&self) -> Vec<(String, Vec<Block>)> {
        self.readings
            .live_book("project_loom", &Agent::Local)
            .await
            .expect("the book should be put together")
            .parts
            .into_iter()
            .filter_map(|part| match part {
                Part::Scene { scene, blocks } => Some((scene.to_string(), blocks)),
                Part::Section { .. } => None,
            })
            .collect()
    }
}

fn settled(scene: &str, words: &str, version: u64) -> Message {
    Message::opening(
        RoutingKey::parse(TEXT_SETTLED).expect("a declared key"),
        serde_json::to_value(SceneTextSettledDTO {
            scene: scene.to_owned(),
            blocks: vec![BlockDTO::Paragraph {
                runs: vec![RunDTO::Text {
                    text: words.to_owned(),
                    marks: MarksDTO::default(),
                }],
            }],
            version,
        })
        .expect("a text is plain data"),
        clock::at(1_700_000_000),
    )
}

fn deleted(scene: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared key"),
        serde_json::to_value(SceneDeletedDTO {
            scene: scene.to_owned(),
        })
        .expect("a deletion is plain data"),
        clock::at(1_700_000_000),
    )
}

fn saying(words: &str) -> Vec<Block> {
    vec![Block::Paragraph(vec![Run::plain(words)])]
}

#[test]
fn it_listens_for_settled_texts_and_for_deletions() {
    // given: the listener
    let bench = a_workbench();

    // expect: it hears a scene's settled text and its deletion
    for routing in [TEXT_SETTLED, DELETED] {
        assert!(
            bench
                .keeping
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn a_book_shows_the_texts_its_scenes_were_told_and_leaves_out_the_others() {
    // given: the listener
    let bench = a_workbench();

    // when: the texts of two of the book's three scenes are heard
    bench
        .hears(settled("scene_harbour", "The boats came in.", 1))
        .await;
    bench
        .hears(settled("scene_stalls", "Fresh today.", 1))
        .await;

    // then: those two are shown, in the book's order, and the third not yet
    assert_eq!(
        bench.scenes().await,
        vec![
            ("scene_harbour".to_owned(), saying("The boats came in.")),
            ("scene_stalls".to_owned(), saying("Fresh today.")),
        ]
    );
}

#[tokio::test]
async fn a_late_text_does_not_undo_a_newer_one() {
    // given: a scene's text heard at a version
    let bench = a_workbench();
    bench
        .hears(settled("scene_harbour", "The boats came in at dawn.", 4))
        .await;

    // when: a text it was told earlier arrives late
    bench
        .hears(settled("scene_harbour", "The boats came in.", 3))
        .await;

    // then: the newer text stands
    assert_eq!(
        bench.scenes().await,
        vec![(
            "scene_harbour".to_owned(),
            saying("The boats came in at dawn.")
        )]
    );
}

#[tokio::test]
async fn a_deleted_scene_is_left_out_for_good() {
    // given: a scene's text heard
    let bench = a_workbench();
    bench
        .hears(settled("scene_harbour", "The boats came in.", 1))
        .await;

    // when: the scene is deleted, and a text of it arrives late
    bench.hears(deleted("scene_harbour")).await;
    bench.hears(settled("scene_harbour", "Too late.", 9)).await;

    // then: the book shows it no more
    assert_eq!(bench.scenes().await, vec![]);
}
