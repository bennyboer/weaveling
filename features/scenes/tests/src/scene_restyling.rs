use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{FakePublisher, Listener, Message, RoutingKey};
use projects_contract::QUOTATION_CHANGED;
use quotation::QuotationMarks;
use scenes_catalog::InMemoryRestylingTasks;
use scenes_contract::{FRAGMENT, MORE_TO_RESTYLE, MoreToRestyleDTO};
use scenes_core::{SceneId, SceneRestyler, SceneService, SceneServiceError, StoreError};
use scenes_messaging::RestyleOnQuotationChanged;
use scenes_store::InMemorySceneStore;
use scenes_sync::LiveScenes;
use serde_json::json;
use time::{Duration, OffsetDateTime};
use yrs::types::Attrs;
use yrs::{
    Any, Doc, ReadTxn, StateVector, Text, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim,
};

const A_PROJECT: &str = "project_1";
const ANOTHER_PROJECT: &str = "project_2";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Vanishing {
    gone: SceneId,
    live: LiveScenes,
}

#[async_trait]
impl SceneRestyler for Vanishing {
    async fn restyle(
        &self,
        scene: SceneId,
        style: QuotationMarks,
    ) -> Result<bool, SceneServiceError> {
        if scene == self.gone {
            return Err(SceneServiceError::Store(StoreError::NotFound(scene)));
        }

        self.live.restyle(scene, style).await
    }
}

struct Wired {
    restyle: RestyleOnQuotationChanged,
    scenes: SceneService,
    live: LiveScenes,
    overheard: Arc<FakePublisher>,
}

fn a_workbench(at_most: usize) -> Wired {
    let scenes = SceneService::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(FixedClock::new(at(1_000))),
    );
    let live = LiveScenes::new(scenes.clone());

    restyling_with(scenes, live.clone(), Arc::new(live), at_most)
}

fn restyling_with(
    scenes: SceneService,
    live: LiveScenes,
    restyler: Arc<dyn SceneRestyler>,
    at_most: usize,
) -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let overheard = Arc::new(FakePublisher::default());

    Wired {
        restyle: RestyleOnQuotationChanged::new(
            scenes.clone(),
            restyler,
            Arc::new(InMemoryRestylingTasks::new()),
            overheard.clone(),
            clock,
        )
        .taking_at_most(at_most),
        scenes,
        live,
        overheard,
    }
}

fn a_dialogue() -> Vec<u8> {
    let doc = Doc::new();
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    {
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        let prose = paragraph.insert(&mut txn, 0, XmlTextPrelim::new(""));
        for (piece, side) in [
            ("»", Some("opening")),
            ("Ja.", None),
            ("«", Some("closing")),
        ] {
            let end = prose.len(&txn);
            let attrs = side.map_or_else(Attrs::new, |side| {
                Attrs::from([(
                    Arc::from("quote"),
                    Any::from(HashMap::from([("side".to_owned(), Any::from(side))])),
                )])
            });
            prose.insert_with_attributes(&mut txn, end, piece, attrs);
        }
    }

    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

impl Wired {
    async fn holding(&self, project: &str, scenes: usize) -> Vec<SceneId> {
        let mut written = Vec::new();

        for _ in 0..scenes {
            let scene = self
                .scenes
                .create(project)
                .await
                .expect("creating a scene should succeed")
                .id();
            self.scenes
                .apply(&scene.to_string(), &a_dialogue())
                .await
                .expect("writing dialogue should succeed");
            written.push(scene);
        }
        written.sort();

        written
    }

    async fn saying(&self, scenes: &[SceneId]) -> Vec<String> {
        let mut said = Vec::new();

        for scene in scenes {
            said.push(
                self.live
                    .join(*scene)
                    .await
                    .expect("the scene should be there")
                    .text(),
            );
        }

        said
    }

    fn carried_on(&self) -> Vec<Message> {
        self.overheard
            .heard()
            .into_iter()
            .filter(|message| message.routing.to_string() == MORE_TO_RESTYLE)
            .collect()
    }

    async fn restyle_through(&self, asked: Message) {
        let mut asked = asked;

        loop {
            let before = self.carried_on().len();
            self.restyle.handle(&asked).await.expect("task succeeds");

            let carried = self.carried_on();
            if carried.len() == before {
                break;
            }

            asked = carried.last().expect("a continuation was sent").clone();
        }
    }
}

fn restyled(project: &str, quotation: &str, change: u64) -> Message {
    Message::opening(
        RoutingKey::parse(QUOTATION_CHANGED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "QUOTATION_CHANGED", "quotation": quotation },
            "aggregate": { "id": project, "kind": "project", "version": change },
            "agent": { "kind": "user", "id": "author-7" },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

fn more_of(project: &str, quotation: &str, change: u64, after: SceneId) -> Message {
    Message::opening(
        RoutingKey::parse(MORE_TO_RESTYLE).expect("a declared routing key is fine"),
        serde_json::to_value(MoreToRestyleDTO {
            project: project.to_owned(),
            quotation: quotation.to_owned(),
            change,
            after: after.to_string(),
        })
        .expect("plain data"),
        at(1_000),
    )
}

#[tokio::test]
async fn a_new_style_reaches_every_scene_of_the_book() {
    let wired = a_workbench(4);
    let book = wired.holding(A_PROJECT, 3).await;

    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("task succeeds");

    assert_eq!(wired.saying(&book).await, vec!["„Ja.“"; 3]);
    assert!(
        wired.carried_on().is_empty(),
        "a batch that did not fill is the last one"
    );
}

#[tokio::test]
async fn a_book_larger_than_one_batch_is_restyled_batch_by_batch() {
    let wired = a_workbench(2);
    let book = wired.holding(A_PROJECT, 5).await;

    wired.restyle_through(restyled(A_PROJECT, "curly", 3)).await;

    assert_eq!(wired.saying(&book).await, vec!["“Ja.”"; 5]);
    assert_eq!(
        wired.carried_on().len(),
        2,
        "five scenes in batches of two is two continuations and a short third batch"
    );
}

#[tokio::test]
async fn a_continuation_carries_the_style_the_change_and_where_it_got_to() {
    let wired = a_workbench(2);
    let book = wired.holding(A_PROJECT, 3).await;
    let asked = restyled(A_PROJECT, "curly", 3);

    wired.restyle.handle(&asked).await.expect("task succeeds");

    let carried = wired.carried_on();
    let more: MoreToRestyleDTO =
        serde_json::from_value(carried[0].payload.clone()).expect("readable");
    assert_eq!(
        more,
        MoreToRestyleDTO {
            project: A_PROJECT.to_owned(),
            quotation: "curly".to_owned(),
            change: 3,
            after: book[1].to_string(),
        }
    );
    assert_eq!(
        carried[0].conversation, asked.conversation,
        "the whole task has to be followable as one thing"
    );
}

#[tokio::test]
async fn another_book_keeps_its_marks() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    let theirs = wired.holding(ANOTHER_PROJECT, 2).await;

    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("task succeeds");

    assert_eq!(wired.saying(&theirs).await, vec!["»Ja.«"; 2]);
}

#[tokio::test]
async fn a_newer_style_stops_an_older_restyling_half_way() {
    let wired = a_workbench(2);
    let book = wired.holding(A_PROJECT, 3).await;
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 5))
        .await
        .expect("task succeeds");

    wired
        .restyle
        .handle(&more_of(A_PROJECT, "low-high", 3, book[0]))
        .await
        .expect("an outdated continuation is not a failure");

    assert_eq!(
        wired.saying(&book[1..]).await,
        vec!["“Ja.”", "»Ja.«"],
        "the older task would otherwise overwrite what the newer one did"
    );
}

#[tokio::test]
async fn an_older_style_heard_late_is_not_begun() {
    let wired = a_workbench(4);
    let book = wired.holding(A_PROJECT, 2).await;
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 5))
        .await
        .expect("task succeeds");

    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("an outdated change is not a failure");

    assert_eq!(wired.saying(&book).await, vec!["“Ja.”"; 2]);
}

#[tokio::test]
async fn the_latest_restyling_carries_on() {
    let wired = a_workbench(1);
    let book = wired.holding(A_PROJECT, 2).await;
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 5))
        .await
        .expect("task succeeds");

    wired
        .restyle
        .handle(&more_of(A_PROJECT, "low-high", 5, book[0]))
        .await
        .expect("task succeeds");

    assert_eq!(wired.saying(&book).await, vec!["„Ja.“"; 2]);
}

#[tokio::test]
async fn hearing_the_same_change_again_goes_through_the_book_again() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 1).await;
    let asked = restyled(A_PROJECT, "curly", 5);
    wired.restyle.handle(&asked).await.expect("task succeeds");
    let written_meanwhile = wired.holding(A_PROJECT, 1).await;

    wired
        .restyle
        .handle(&asked)
        .await
        .expect("a redelivery is not a failure");

    assert_eq!(
        wired.saying(&written_meanwhile).await,
        vec!["“Ja.”"],
        "a change delivered again, after a crash half way, must still finish the book"
    );
}

#[tokio::test]
async fn a_continuation_nobody_remembers_beginning_carries_on() {
    let wired = a_workbench(1);
    let book = wired.holding(A_PROJECT, 2).await;

    wired
        .restyle
        .handle(&more_of(A_PROJECT, "curly", 5, book[0]))
        .await
        .expect("task succeeds");

    assert_eq!(
        wired.saying(&book[1..]).await,
        vec!["“Ja.”"],
        "a restart forgets what was begun, and the book must not stay half restyled"
    );
}

#[tokio::test]
async fn a_scene_deleted_meanwhile_is_passed_over() {
    let plain = a_workbench(4);
    let book = plain.holding(A_PROJECT, 2).await;
    let wired = restyling_with(
        plain.scenes.clone(),
        plain.live.clone(),
        Arc::new(Vanishing {
            gone: book[0],
            live: plain.live.clone(),
        }),
        4,
    );

    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 3))
        .await
        .expect("a scene deleted while the book is restyled is no failure");

    assert_eq!(wired.saying(&book[1..]).await, vec!["“Ja.”"]);
}

#[tokio::test]
async fn a_style_nobody_knows_is_refused() {
    let wired = a_workbench(4);

    assert!(
        wired
            .restyle
            .handle(&restyled(A_PROJECT, "guillemets", 3))
            .await
            .is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}

#[tokio::test]
async fn a_message_about_no_project_at_all_is_refused() {
    let wired = a_workbench(4);
    let nonsense = Message::opening(
        RoutingKey::parse(QUOTATION_CHANGED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(wired.restyle.handle(&nonsense).await.is_err());
}

#[tokio::test]
async fn a_project_event_that_changes_no_style_is_refused() {
    let wired = a_workbench(4);
    let renamed = Message::opening(
        RoutingKey::parse(QUOTATION_CHANGED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "RENAMED", "project_name": "Another title" },
            "aggregate": { "id": A_PROJECT, "kind": "project", "version": 3 },
            "agent": { "kind": "user", "id": "author-7" },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    );

    assert!(wired.restyle.handle(&renamed).await.is_err());
}
