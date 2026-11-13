use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::AllowingEverything;
use belonging::InMemoryBelongings;
use clock::FixedClock;
use eventsourcing::Agent;
use messaging::{FakePublisher, Listener, Message, RoutingKey};
use projects_contract::QUOTATION_CHANGED;
use quotation::QuotationMarks;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_contract::{FRAGMENT, MORE_QUOTATION_MARKS_TO_RESTYLE, MoreQuotationMarksToRestyleDTO};
use scenes_core::{
    ProjectLink, QuotationRestylingProgress, QuotationRestylingTasks, SceneId, SceneRestyler,
    ScenesCore, ScenesError, StoreError,
};
use scenes_messaging::RestyleOnQuotationChangedListener;
use scenes_store::InMemorySceneStore;
use scenes_sync::LiveScenes;
use serde_json::json;
use transacting::InMemoryTransactions;
use yrs::types::Attrs;
use yrs::{
    Any, Doc, ReadTxn, StateVector, Text, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim,
};

const A_PROJECT: &str = "project_1";
const ANOTHER_PROJECT: &str = "project_2";

struct Vanishing {
    gone: SceneId,
    live: LiveScenes,
}

#[async_trait]
impl SceneRestyler for Vanishing {
    async fn restyle(&self, scene: SceneId, style: QuotationMarks) -> Result<bool, ScenesError> {
        if scene == self.gone {
            return Err(ScenesError::Store(StoreError::NotFound(scene)));
        }

        self.live.restyle(scene, style).await
    }
}

struct Broken;

#[async_trait]
impl SceneRestyler for Broken {
    async fn restyle(&self, _scene: SceneId, _style: QuotationMarks) -> Result<bool, ScenesError> {
        Err(ScenesError::Store(StoreError::Backend(
            "the store is down".into(),
        )))
    }
}

struct Overtaken {
    live: LiveScenes,
    copy: Mutex<Option<(Wired, Message)>>,
}

#[async_trait]
impl SceneRestyler for Overtaken {
    async fn restyle(&self, scene: SceneId, style: QuotationMarks) -> Result<bool, ScenesError> {
        let copy = self.copy.lock().expect("never poisoned").take();
        if let Some((copy, asked)) = copy {
            copy.restyle
                .handle(&asked)
                .await
                .expect("the copy of the page restyles it whole");
        }

        self.live.restyle(scene, style).await
    }
}

struct Wired {
    restyle: RestyleOnQuotationChangedListener,
    scenes: ScenesCore,
    live: LiveScenes,
    tasks: Arc<InMemoryQuotationRestylingTasks>,
    overheard: Arc<FakePublisher>,
}

fn a_workbench(at_most: usize) -> Wired {
    let tasks = Arc::new(InMemoryQuotationRestylingTasks::new());
    let scenes = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        tasks.clone(),
        Arc::new(InMemoryBelongings::new()),
        crate::kept::heard_of_nobody(),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_000))),
    );
    let live = LiveScenes::new(scenes.clone());

    restyling_with(scenes, tasks, live.clone(), Arc::new(live), at_most)
}

fn restyling_with(
    scenes: ScenesCore,
    tasks: Arc<InMemoryQuotationRestylingTasks>,
    live: LiveScenes,
    restyler: Arc<dyn SceneRestyler>,
    at_most: usize,
) -> Wired {
    let clock = Arc::new(FixedClock::new(clock::at(1_000)));
    let overheard = Arc::new(FakePublisher::default());

    Wired {
        restyle: RestyleOnQuotationChangedListener::new(
            scenes.clone(),
            restyler,
            overheard.clone(),
            clock,
        )
        .taking_at_most(at_most),
        scenes,
        live,
        tasks,
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
                .create(project, &Agent::Local)
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

    async fn texts_of(&self, scenes: &[SceneId]) -> Vec<String> {
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

    async fn progress(&self) -> Option<QuotationRestylingProgress> {
        self.tasks
            .progress(&ProjectLink::from(A_PROJECT))
            .await
            .expect("reading progress should succeed")
    }

    fn continuations(&self) -> Vec<Message> {
        self.overheard
            .heard()
            .into_iter()
            .filter(|message| message.routing.to_string() == MORE_QUOTATION_MARKS_TO_RESTYLE)
            .collect()
    }

    async fn restyle_through(&self, asked: Message) {
        let mut asked = asked;

        loop {
            let before = self.continuations().len();
            self.restyle
                .handle(&asked)
                .await
                .expect("restyling succeeds");

            let carried = self.continuations();
            if carried.len() == before {
                break;
            }

            asked = carried.last().expect("a continuation was sent").clone();
        }
    }
}

fn restyled(project: &str, quotation: &str, change: u64) -> Message {
    eventpublishing::published(
        QUOTATION_CHANGED,
        "project",
        project,
        change,
        json!({ "version": 0, "name": "QUOTATION_CHANGED", "quotation": quotation }),
        &eventsourcing::an_author(),
        clock::at(1_000),
    )
}

fn more_of(project: &str, quotation: &str, change: u64, after: SceneId) -> Message {
    Message::opening(
        RoutingKey::parse(MORE_QUOTATION_MARKS_TO_RESTYLE).expect("a declared routing key is fine"),
        serde_json::to_value(MoreQuotationMarksToRestyleDTO {
            project: project.to_owned(),
            quotation: quotation.to_owned(),
            change,
            after: after.to_string(),
        })
        .expect("plain data"),
        clock::at(1_000),
    )
}

#[tokio::test]
async fn a_new_style_reaches_every_scene_of_the_book() {
    // given: a book smaller than a batch
    let wired = a_workbench(4);
    let book = wired.holding(A_PROJECT, 3).await;

    // when: a new style is heard
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("restyling succeeds");

    // then: every scene has it
    assert_eq!(wired.texts_of(&book).await, vec!["„Ja.“"; 3]);
    // and: the restyling does not carry on
    assert!(
        wired.continuations().is_empty(),
        "a batch that did not fill is the last one"
    );
}

#[tokio::test]
async fn a_book_larger_than_one_batch_is_restyled_batch_by_batch() {
    // given: a book of several batches
    let wired = a_workbench(2);
    let book = wired.holding(A_PROJECT, 5).await;

    // when: a new style is restyled through
    wired.restyle_through(restyled(A_PROJECT, "curly", 3)).await;

    // then: every scene has it
    assert_eq!(wired.texts_of(&book).await, vec!["“Ja.”"; 5]);
    // and: it carried on once per full batch
    assert_eq!(
        wired.continuations().len(),
        2,
        "five scenes in batches of two is two continuations and a short third batch"
    );
}

#[tokio::test]
async fn a_continuation_carries_the_style_the_change_and_where_it_got_to() {
    // given: a book larger than a batch
    let wired = a_workbench(2);
    let book = wired.holding(A_PROJECT, 3).await;
    let asked = restyled(A_PROJECT, "low-high", 3);

    // when: a new style is heard
    wired
        .restyle
        .handle(&asked)
        .await
        .expect("restyling succeeds");

    // then: the continuation carries the style, the change and the last scene restyled
    let carried = wired.continuations();
    let more: MoreQuotationMarksToRestyleDTO =
        serde_json::from_value(carried[0].payload.clone()).expect("readable");
    assert_eq!(
        more,
        MoreQuotationMarksToRestyleDTO {
            project: A_PROJECT.to_owned(),
            quotation: "low-high".to_owned(),
            change: 3,
            after: book[1].to_string(),
        }
    );
    // and: it stays in the change's conversation
    assert_eq!(
        carried[0].conversation, asked.conversation,
        "the whole restyling has to be followable as one thing"
    );
}

#[tokio::test]
async fn another_book_keeps_its_marks() {
    // given: two books
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    let theirs = wired.holding(ANOTHER_PROJECT, 2).await;

    // when: a new style of one is heard
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("restyling succeeds");

    // then: the other keeps its marks
    assert_eq!(wired.texts_of(&theirs).await, vec!["»Ja.«"; 2]);
}

#[tokio::test]
async fn a_newer_style_stops_an_older_restyling_half_way() {
    // given: a book larger than a batch
    let wired = a_workbench(2);
    let book = wired.holding(A_PROJECT, 3).await;
    // and: a style begun
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("restyling succeeds");
    // and: a newer style begun after it
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 5))
        .await
        .expect("restyling succeeds");

    // when: the older restyling's continuation is heard
    wired
        .restyle
        .handle(&more_of(A_PROJECT, "low-high", 3, book[0]))
        .await
        .expect("an outdated continuation is not a failure");

    // then: the rest of the book keeps what it had
    assert_eq!(
        wired.texts_of(&book[1..]).await,
        vec!["“Ja.”", "»Ja.«"],
        "the older restyling would otherwise overwrite what the newer one did"
    );
}

#[tokio::test]
async fn an_older_style_heard_late_is_not_begun() {
    // given: a book restyled
    let wired = a_workbench(4);
    let book = wired.holding(A_PROJECT, 2).await;
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 5))
        .await
        .expect("restyling succeeds");

    // when: an older style is heard late
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("an outdated change is not a failure");

    // then: the book keeps the newer style
    assert_eq!(wired.texts_of(&book).await, vec!["“Ja.”"; 2]);
}

#[tokio::test]
async fn the_latest_restyling_carries_on() {
    // given: a book larger than a batch
    let wired = a_workbench(1);
    let book = wired.holding(A_PROJECT, 2).await;
    // and: its latest style begun
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 5))
        .await
        .expect("restyling succeeds");

    // when: its continuation is heard
    wired
        .restyle
        .handle(&more_of(A_PROJECT, "low-high", 5, book[0]))
        .await
        .expect("restyling succeeds");

    // then: the whole book has the style
    assert_eq!(wired.texts_of(&book).await, vec!["„Ja.“"; 2]);
}

#[tokio::test]
async fn a_continuation_nobody_remembers_beginning_carries_on() {
    // given: a book nobody remembers restyling
    let wired = a_workbench(1);
    let book = wired.holding(A_PROJECT, 2).await;

    // when: a continuation of a restyling is heard
    wired
        .restyle
        .handle(&more_of(A_PROJECT, "curly", 5, book[0]))
        .await
        .expect("restyling succeeds");

    // then: the rest of the book is restyled
    assert_eq!(
        wired.texts_of(&book[1..]).await,
        vec!["“Ja.”"],
        "a restart forgets what was begun, and the book must not stay half restyled"
    );
}

#[tokio::test]
async fn a_scene_deleted_meanwhile_is_passed_over() {
    // given: a book
    let plain = a_workbench(4);
    let book = plain.holding(A_PROJECT, 2).await;
    // and: one of its scenes deleted while it is restyled
    let wired = restyling_with(
        plain.scenes.clone(),
        plain.tasks.clone(),
        plain.live.clone(),
        Arc::new(Vanishing {
            gone: book[0],
            live: plain.live.clone(),
        }),
        4,
    );

    // when: a new style is heard
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 3))
        .await
        .expect("a scene deleted while the book is restyled is no failure");

    // then: the rest of the book has it
    assert_eq!(wired.texts_of(&book[1..]).await, vec!["“Ja.”"]);
}

#[tokio::test]
async fn a_scene_that_cannot_be_restyled_fails_the_batch_so_it_is_tried_again() {
    // given: a book
    let plain = a_workbench(4);
    plain.holding(A_PROJECT, 2).await;
    // and: its scenes cannot be restyled
    let wired = restyling_with(
        plain.scenes.clone(),
        plain.tasks.clone(),
        plain.live.clone(),
        Arc::new(Broken),
        4,
    );

    // then: hearing a new style fails
    assert!(
        wired
            .restyle
            .handle(&restyled(A_PROJECT, "curly", 3))
            .await
            .is_err(),
        "only a scene deleted meanwhile may be passed over; any other failure leaves the book half restyled"
    );
}

#[tokio::test]
async fn a_style_nobody_knows_is_refused() {
    // given: a listener for new styles
    let wired = a_workbench(4);

    // then: hearing a style nobody knows fails
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
    // given: a message about no project at all
    let wired = a_workbench(4);
    let nonsense = Message::opening(
        RoutingKey::parse(QUOTATION_CHANGED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        clock::at(1_000),
    );

    // then: hearing it fails
    assert!(wired.restyle.handle(&nonsense).await.is_err());
}

#[tokio::test]
async fn a_project_event_that_changes_no_style_is_refused() {
    // given: a project event that changes no style
    let wired = a_workbench(4);
    let renamed = eventpublishing::published(
        QUOTATION_CHANGED,
        "project",
        A_PROJECT,
        3,
        json!({ "version": 0, "name": "RENAMED", "project_name": "Another title" }),
        &eventsourcing::an_author(),
        clock::at(1_000),
    );

    // then: hearing it fails
    assert!(wired.restyle.handle(&renamed).await.is_err());
}

#[tokio::test]
async fn a_continuation_heard_after_a_restart_still_turns_an_older_style_away() {
    // given: a book whose newer restyling was carried on after a restart
    let wired = a_workbench(1);
    let book = wired.holding(A_PROJECT, 2).await;
    wired
        .restyle
        .handle(&more_of(A_PROJECT, "curly", 5, book[0]))
        .await
        .expect("restyling succeeds");

    // when: an older style is heard
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "low-high", 3))
        .await
        .expect("an outdated change is not a failure");

    // then: the book keeps the newer style
    assert_eq!(
        wired.texts_of(&book[1..]).await,
        vec!["“Ja.”"],
        "the continuation tells of a newer change, so the older one must not begin"
    );
}

fn so_far(total: u64, done: u64, finished: bool) -> Option<QuotationRestylingProgress> {
    Some(QuotationRestylingProgress {
        change: 3,
        total,
        done,
        finished,
    })
}

#[tokio::test]
async fn a_restyling_counts_the_book_before_it_begins() {
    // given: a book of several batches
    let wired = a_workbench(2);
    wired.holding(A_PROJECT, 5).await;
    // and: another book
    wired.holding(ANOTHER_PROJECT, 4).await;

    // when: a new style of the first is heard
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 3))
        .await
        .expect("restyling succeeds");

    // then: its progress counts its own scenes and the first batch done
    assert_eq!(
        wired.progress().await,
        so_far(5, 2, false),
        "five scenes counted in pages of two, and the first batch of two done"
    );
}

#[tokio::test]
async fn a_restyling_is_finished_once_its_last_batch_is_done() {
    // given: a book whose last batch is short
    let wired = a_workbench(2);
    wired.holding(A_PROJECT, 5).await;

    // when: a new style is restyled through
    wired.restyle_through(restyled(A_PROJECT, "curly", 3)).await;

    // then: the restyling is finished
    assert_eq!(wired.progress().await, so_far(5, 5, true));
}

#[tokio::test]
async fn a_book_whose_last_batch_is_full_is_finished_by_the_empty_one_after() {
    // given: a book whose last batch is full
    let wired = a_workbench(2);
    wired.holding(A_PROJECT, 4).await;

    // when: a new style is restyled through
    wired.restyle_through(restyled(A_PROJECT, "curly", 3)).await;

    // then: the restyling is finished
    assert_eq!(wired.progress().await, so_far(4, 4, true));
}

#[tokio::test]
async fn a_book_without_scenes_is_finished_at_once() {
    // given: a book without scenes
    let wired = a_workbench(2);

    // when: a new style is heard
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 3))
        .await
        .expect("restyling succeeds");

    // then: the restyling is finished
    assert_eq!(wired.progress().await, so_far(0, 0, true));
}

#[tokio::test]
async fn a_change_heard_again_carries_on_where_it_got_and_counts_each_scene_once() {
    // given: a book larger than a batch
    let wired = a_workbench(1);
    let book = wired.holding(A_PROJECT, 2).await;
    // and: a new style heard
    let asked = restyled(A_PROJECT, "curly", 3);
    wired
        .restyle
        .handle(&asked)
        .await
        .expect("restyling succeeds");
    let first = wired.continuations();

    // when: it is heard again
    wired
        .restyle
        .handle(&asked)
        .await
        .expect("a redelivery is not a failure");

    // then: it carries on with the continuation it sent before
    let again = wired.continuations();
    assert_eq!(
        again.len(),
        2,
        "heard again, it carries on from where it got"
    );
    assert_eq!(
        again[1].id, first[0].id,
        "it carries on with the continuation it sent before, so a second chain cannot fork"
    );
    // and: the first batch is counted once
    assert_eq!(wired.progress().await, so_far(2, 1, false));

    // when: it is restyled through
    wired.restyle_through(again[1].clone()).await;

    // then: the whole book has the style
    assert_eq!(wired.texts_of(&book).await, vec!["“Ja.”"; 2]);
    // and: each scene is counted once
    assert_eq!(wired.progress().await, so_far(2, 2, true));
}

#[tokio::test]
async fn a_page_heard_twice_is_counted_once() {
    // given: a book of several batches
    let wired = a_workbench(1);
    wired.holding(A_PROJECT, 3).await;
    // and: its restyling begun
    wired
        .restyle
        .handle(&restyled(A_PROJECT, "curly", 3))
        .await
        .expect("restyling succeeds");
    let next = wired.continuations()[0].clone();

    // when: its next page is heard twice
    for _ in 0..2 {
        wired
            .restyle
            .handle(&next)
            .await
            .expect("a page heard again is not a failure");
    }

    // then: the page is counted once
    assert_eq!(wired.progress().await, so_far(3, 2, false));
    // and: both hearings carry on with the same continuation
    let carried = wired.continuations();
    assert_eq!(carried.len(), 3);
    assert_eq!(
        carried[1].id, carried[2].id,
        "the page heard again answers with the same continuation"
    );
}

#[tokio::test]
async fn a_restyling_whose_page_keeps_failing_stays_unfinished_until_it_is_tried_again() {
    // given: a book
    let plain = a_workbench(4);
    let book = plain.holding(A_PROJECT, 2).await;
    // and: its scenes cannot be restyled for now
    let broken = restyling_with(
        plain.scenes.clone(),
        plain.tasks.clone(),
        plain.live.clone(),
        Arc::new(Broken),
        4,
    );
    let asked = restyled(A_PROJECT, "curly", 3);

    // then: hearing a new style fails
    assert!(broken.restyle.handle(&asked).await.is_err());
    // and: the restyling stays unfinished
    assert_eq!(
        plain.progress().await,
        so_far(2, 0, false),
        "a page that failed is not counted, so the bar shows the book unfinished"
    );

    // when: it is tried again once the scenes can be restyled
    plain
        .restyle
        .handle(&asked)
        .await
        .expect("the dead letter tried again succeeds");

    // then: the whole book has the style
    assert_eq!(plain.texts_of(&book).await, vec!["“Ja.”"; 2]);
    // and: the restyling is finished
    assert_eq!(plain.progress().await, so_far(2, 2, true));
}

#[tokio::test]
async fn a_page_overtaken_by_a_copy_of_itself_leaves_carrying_on_to_the_copy() {
    // given: a book larger than a batch
    let plain = a_workbench(1);
    plain.holding(A_PROJECT, 2).await;
    // and: a copy of its first page that overtakes it
    let asked = restyled(A_PROJECT, "curly", 3);
    let wired = restyling_with(
        plain.scenes.clone(),
        plain.tasks.clone(),
        plain.live.clone(),
        Arc::new(Overtaken {
            live: plain.live.clone(),
            copy: Mutex::new(Some((plain, asked.clone()))),
        }),
        1,
    );

    // when: the page is heard
    wired
        .restyle
        .handle(&asked)
        .await
        .expect("a page overtaken is no failure");

    // then: it does not carry on itself
    assert!(
        wired.continuations().is_empty(),
        "the copy counted the page and carries on, so the one it overtook must not fork a second chain"
    );
    // and: the page is counted once
    assert_eq!(wired.progress().await, so_far(2, 1, false));
}
