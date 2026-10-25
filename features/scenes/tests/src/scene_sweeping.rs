use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use messaging::{FakePublisher, Listener, Message, RoutingKey};
use projects_contract::DELETED;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use scenes_core::{
    IdeaLink, ProjectLink, Scene, SceneId, SceneService, SceneStore, SceneTitle, ScenesCore,
    StoreError,
};
use scenes_messaging::DeleteOnProjectDeletedListener;
use scenes_store::InMemorySceneStore;
use serde_json::json;
use time::{Duration, OffsetDateTime};

const A_PROJECT: &str = "project_1";
const ANOTHER_PROJECT: &str = "project_2";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct TheSystemAndAda;

fn ada() -> Agent {
    Agent::Account(AccountId::from("account_ada"))
}

#[async_trait]
impl Authorizer for TheSystemAndAda {
    async fn may(&self, agent: &Agent, _act: Act, _project: &str) -> Result<bool, AuthorizerError> {
        Ok(*agent == Agent::System || *agent == ada())
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

struct Wavering {
    held: InMemorySceneStore,
    gone_meanwhile: Mutex<Option<SceneId>>,
    failing: AtomicBool,
}

#[async_trait]
impl SceneStore for Wavering {
    async fn create(&self, scene: &Scene) -> Result<(), StoreError> {
        self.held.create(scene).await
    }

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError> {
        self.held.load(id).await
    }

    async fn apply(&self, id: SceneId, update: &[u8]) -> Result<(), StoreError> {
        self.held.apply(id, update).await
    }

    async fn retitle(&self, id: SceneId, title: &SceneTitle) -> Result<(), StoreError> {
        self.held.retitle(id, title).await
    }

    async fn announce_word_count(&self, id: SceneId, words: usize) -> Result<(), StoreError> {
        self.held.announce_word_count(id, words).await
    }

    async fn announce_opening(&self, id: SceneId, opening: &str) -> Result<(), StoreError> {
        self.held.announce_opening(id, opening).await
    }

    async fn link(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.link(id, idea).await
    }

    async fn unlink(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.unlink(id, idea).await
    }

    async fn unlink_everywhere(&self, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.unlink_everywhere(idea).await
    }

    async fn delete(&self, id: SceneId) -> Result<(), StoreError> {
        if self.failing.load(Ordering::Acquire) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.delete(id).await
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, StoreError> {
        let mut found = self.held.in_project(project, after, at_most).await?;
        if let Some(gone) = *self.gone_meanwhile.lock().expect("never poisoned") {
            found.insert(0, gone);
        }

        Ok(found)
    }
}

struct Wired {
    sweep: DeleteOnProjectDeletedListener,
    scenes: ScenesCore,
    store: Arc<Wavering>,
    kept: SceneService,
    overheard: Arc<FakePublisher>,
}

fn a_workbench(at_most: usize) -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let overheard = Arc::new(FakePublisher::default());
    let store = Arc::new(Wavering {
        held: InMemorySceneStore::new(),
        gone_meanwhile: Mutex::new(None),
        failing: AtomicBool::new(false),
    });
    let scenes = ScenesCore::new(
        store.clone(),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(TheSystemAndAda),
        clock.clone(),
    );

    Wired {
        sweep: DeleteOnProjectDeletedListener::new(scenes.clone(), overheard.clone(), clock)
            .taking_at_most(at_most),
        scenes,
        kept: SceneService::new(store.clone(), Arc::new(FixedClock::new(at(1_000)))),
        store,
        overheard,
    }
}

impl Wired {
    async fn holding(&self, project: &str, scenes: usize) -> Vec<SceneId> {
        let mut written = Vec::new();

        for _ in 0..scenes {
            written.push(
                self.scenes
                    .create(project, &ada())
                    .await
                    .expect("creating a scene should succeed")
                    .id(),
            );
        }
        written.sort();

        written
    }

    async fn still_there(&self, project: &str) -> Vec<SceneId> {
        self.kept
            .in_project(project, None, 1_000)
            .await
            .expect("listing should succeed")
    }

    fn carried_on(&self) -> Vec<Message> {
        self.overheard
            .heard()
            .into_iter()
            .filter(|message| message.routing.to_string() == MORE_TO_SWEEP)
            .collect()
    }

    async fn sweep_through(&self, asked: Message) {
        let mut asked = asked;

        loop {
            let before = self.carried_on().len();
            self.sweep.handle(&asked).await.expect("sweeping succeeds");

            let carried = self.carried_on();
            if carried.len() == before {
                break;
            }

            asked = carried.last().expect("a continuation was sent").clone();
        }
    }
}

fn deleted(project: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DELETED" },
            "aggregate": { "id": project, "kind": "project", "version": 2 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

#[tokio::test]
async fn deleting_a_project_takes_its_prose_with_it() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 3).await;

    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    assert!(
        wired.still_there(A_PROJECT).await.is_empty(),
        "the CRDT store is the only copy of the prose, so a deleted project has to reach it \
         or the words outlive everything that could ever find them again"
    );
}

#[tokio::test]
async fn a_scene_gone_before_the_sweep_reaches_it_is_passed_over() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    *wired.store.gone_meanwhile.lock().expect("never poisoned") = Some(SceneId::generate(at(500)));

    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("a scene deleted by someone else meanwhile is no reason to fail");

    *wired.store.gone_meanwhile.lock().expect("never poisoned") = None;
    assert!(wired.still_there(A_PROJECT).await.is_empty());
}

#[tokio::test]
async fn a_store_failing_while_sweeping_fails_the_sweep_so_it_is_tried_again() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    wired.store.failing.store(true, Ordering::Release);

    assert!(
        wired.sweep.handle(&deleted(A_PROJECT)).await.is_err(),
        "a sweep that quietly skipped what it could not delete would leave prose behind for good"
    );
}

#[tokio::test]
async fn a_project_smaller_than_one_batch_does_not_carry_on() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 3).await;

    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    assert!(
        wired.carried_on().is_empty(),
        "a batch that did not fill is the last one, and carrying on would never end"
    );
}

#[tokio::test]
async fn a_project_larger_than_one_batch_is_swept_batch_by_batch() {
    let wired = a_workbench(2);
    wired.holding(A_PROJECT, 7).await;

    wired.sweep_through(deleted(A_PROJECT)).await;

    assert!(
        wired.still_there(A_PROJECT).await.is_empty(),
        "a sweep bounded at two scenes must carry itself on until the project is empty, \
         or a book of thousands is swept only as far as its first batch"
    );
    assert_eq!(
        wired.carried_on().len(),
        3,
        "seven scenes in batches of two is three continuations and a short fourth batch"
    );
}

#[tokio::test]
async fn a_continuation_stays_in_the_conversation_the_deletion_opened() {
    let wired = a_workbench(2);
    wired.holding(A_PROJECT, 4).await;
    let asked = deleted(A_PROJECT);

    wired.sweep.handle(&asked).await.expect("sweeping succeeds");

    let carried = wired.carried_on();
    assert_eq!(carried.len(), 1);
    assert_eq!(
        carried[0].conversation, asked.conversation,
        "the whole sweep has to be followable as one thing, however many batches it takes"
    );
    assert_eq!(carried[0].caused_by, Some(asked.id));
}

#[tokio::test]
async fn a_continuation_starts_after_the_scene_it_names() {
    let wired = a_workbench(2);
    let written = wired.holding(A_PROJECT, 4).await;

    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    let asked: MoreToSweepDTO =
        serde_json::from_value(wired.carried_on()[0].payload.clone()).expect("readable");
    assert_eq!(asked.project, A_PROJECT);
    assert_eq!(
        asked.after,
        written[1].to_string(),
        "the cursor names the last scene of the batch just swept"
    );
}

#[tokio::test]
async fn another_project_keeps_its_prose() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    let theirs = wired.holding(ANOTHER_PROJECT, 2).await;

    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    assert_eq!(
        wired.still_there(ANOTHER_PROJECT).await,
        theirs,
        "one deleted project must never take another author's book with it"
    );
}

#[tokio::test]
async fn hearing_the_same_deletion_twice_is_harmless() {
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 3).await;

    for _ in 0..2 {
        wired
            .sweep
            .handle(&deleted(A_PROJECT))
            .await
            .expect("a redelivery is not a failure");
    }

    assert!(wired.still_there(A_PROJECT).await.is_empty());
}

#[tokio::test]
async fn a_message_about_no_project_at_all_is_refused() {
    let wired = a_workbench(4);
    let nonsense = Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(
        wired.sweep.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}
