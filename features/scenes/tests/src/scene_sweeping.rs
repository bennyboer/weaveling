use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    IdeaLink, ProjectLink, Scene, SceneId, SceneStore, SceneTitle, ScenesCore, ScenesError,
    StoreError,
};
use scenes_messaging::DeleteOnProjectDeletedListener;
use scenes_store::InMemorySceneStore;
use serde_json::json;
use transacting::{InMemoryTransactions, Transaction};

const A_PROJECT: &str = "project_1";
const ANOTHER_PROJECT: &str = "project_2";

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
    created: AtomicUsize,
}

#[async_trait]
impl SceneStore for Wavering {
    async fn create(&self, transaction: &mut Transaction, scene: &Scene) -> Result<(), StoreError> {
        self.created.fetch_add(1, Ordering::AcqRel);
        self.held.create(transaction, scene).await
    }

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError> {
        self.held.load(id).await
    }

    async fn apply(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        update: &[u8],
    ) -> Result<(), StoreError> {
        self.held.apply(transaction, id, update).await
    }

    async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        title: &SceneTitle,
    ) -> Result<(), StoreError> {
        self.held.retitle(transaction, id, title).await
    }

    async fn announce_word_count(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        words: usize,
        as_of: u64,
    ) -> Result<(), StoreError> {
        self.held
            .announce_word_count(transaction, id, words, as_of)
            .await
    }

    async fn announce_opening(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        opening: &str,
    ) -> Result<(), StoreError> {
        self.held.announce_opening(transaction, id, opening).await
    }

    async fn link(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.held.link(transaction, id, idea).await
    }

    async fn unlink(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.held.unlink(transaction, id, idea).await
    }

    async fn unlink_everywhere(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.held.unlink_everywhere(transaction, idea).await
    }

    async fn delete(&self, transaction: &mut Transaction, id: SceneId) -> Result<(), StoreError> {
        if self.failing.load(Ordering::Acquire) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.delete(transaction, id).await
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

    async fn version_of(&self, id: SceneId) -> Result<u64, StoreError> {
        self.held.version_of(id).await
    }

    async fn prose_version_of(&self, id: SceneId) -> Result<u64, StoreError> {
        self.held.prose_version_of(id).await
    }

    async fn version_within(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
    ) -> Result<u64, StoreError> {
        self.held.version_within(transaction, id).await
    }

    async fn uncounted(&self, at_most: usize) -> Result<Vec<SceneId>, StoreError> {
        self.held.uncounted(at_most).await
    }
}

struct Wired {
    sweep: DeleteOnProjectDeletedListener,
    scenes: ScenesCore,
    store: Arc<Wavering>,
    overheard: Arc<FakePublisher>,
}

fn a_workbench(at_most: usize) -> Wired {
    let clock = Arc::new(FixedClock::new(clock::at(1_000)));
    let overheard = Arc::new(FakePublisher::default());
    let store = Arc::new(Wavering {
        held: InMemorySceneStore::new(),
        gone_meanwhile: Mutex::new(None),
        failing: AtomicBool::new(false),
        created: AtomicUsize::new(0),
    });
    let scenes = ScenesCore::new(
        store.clone(),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(TheSystemAndAda),
        clock.clone(),
    );

    Wired {
        sweep: DeleteOnProjectDeletedListener::new(scenes.clone(), overheard.clone(), clock)
            .taking_at_most(at_most),
        scenes,
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
        self.store
            .held
            .in_project(&ProjectLink::from(project), None, 1_000)
            .await
            .expect("listing should succeed")
    }

    fn continuations(&self) -> Vec<Message> {
        self.overheard
            .heard()
            .into_iter()
            .filter(|message| message.routing.to_string() == MORE_TO_SWEEP)
            .collect()
    }

    async fn sweep_through(&self, asked: Message) {
        let mut asked = asked;

        loop {
            let before = self.continuations().len();
            self.sweep.handle(&asked).await.expect("sweeping succeeds");

            let carried = self.continuations();
            if carried.len() == before {
                break;
            }

            asked = carried.last().expect("a continuation was sent").clone();
        }
    }
}

fn deleted(project: &str) -> Message {
    eventpublishing::published(
        DELETED,
        "project",
        project,
        2,
        json!({ "version": 0, "name": "DELETED" }),
        &Agent::System,
        clock::at(1_000),
    )
}

#[tokio::test]
async fn deleting_a_project_takes_its_prose_with_it() {
    // given: a project holding scenes
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 3).await;

    // when: its deletion is heard
    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    // then: none of its scenes is left
    assert!(
        wired.still_there(A_PROJECT).await.is_empty(),
        "the CRDT store is the only copy of the prose, so a deleted project has to reach it \
         or the words outlive everything that could ever find them again"
    );
}

#[tokio::test]
async fn a_scene_gone_before_the_sweep_reaches_it_is_passed_over() {
    // given: a project holding scenes
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    // and: one of them gone before the sweep reaches it
    *wired.store.gone_meanwhile.lock().expect("never poisoned") =
        Some(SceneId::generate(clock::at(500)));

    // when: the project's deletion is heard
    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("a scene deleted by someone else meanwhile is no reason to fail");

    // then: the rest are gone too
    *wired.store.gone_meanwhile.lock().expect("never poisoned") = None;
    assert!(wired.still_there(A_PROJECT).await.is_empty());
}

#[tokio::test]
async fn a_store_failing_while_sweeping_fails_the_sweep_so_it_is_tried_again() {
    // given: a project holding scenes
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    // and: a store failing to delete
    wired.store.failing.store(true, Ordering::Release);

    // then: hearing the project's deletion fails
    assert!(
        wired.sweep.handle(&deleted(A_PROJECT)).await.is_err(),
        "a sweep that quietly skipped what it could not delete would leave prose behind for good"
    );
}

#[tokio::test]
async fn a_project_smaller_than_one_batch_does_not_carry_on() {
    // given: a project holding fewer scenes than a batch
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 3).await;

    // when: its deletion is heard
    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    // then: the sweep does not carry on
    assert!(
        wired.continuations().is_empty(),
        "a batch that did not fill is the last one, and carrying on would never end"
    );
}

#[tokio::test]
async fn a_project_larger_than_one_batch_is_swept_batch_by_batch() {
    // given: a project holding several batches of scenes
    let wired = a_workbench(2);
    wired.holding(A_PROJECT, 7).await;

    // when: its deletion is swept through
    wired.sweep_through(deleted(A_PROJECT)).await;

    // then: none of its scenes is left
    assert!(
        wired.still_there(A_PROJECT).await.is_empty(),
        "a sweep bounded at two scenes must carry itself on until the project is empty, \
         or a book of thousands is swept only as far as its first batch"
    );
    // and: it carried on once per full batch
    assert_eq!(
        wired.continuations().len(),
        3,
        "seven scenes in batches of two is three continuations and a short fourth batch"
    );
}

#[tokio::test]
async fn a_continuation_stays_in_the_conversation_the_deletion_opened() {
    // given: a project holding more than a batch
    let wired = a_workbench(2);
    wired.holding(A_PROJECT, 4).await;
    let asked = deleted(A_PROJECT);

    // when: its deletion is heard
    wired.sweep.handle(&asked).await.expect("sweeping succeeds");

    // then: the sweep carries on in the deletion's conversation
    let carried = wired.continuations();
    assert_eq!(carried.len(), 1);
    assert_eq!(
        carried[0].conversation, asked.conversation,
        "the whole sweep has to be followable as one thing, however many batches it takes"
    );
    // and: caused by the deletion
    assert_eq!(carried[0].caused_by, Some(asked.id));
}

#[tokio::test]
async fn a_continuation_starts_after_the_scene_it_names() {
    // given: a project holding more than a batch
    let wired = a_workbench(2);
    let written = wired.holding(A_PROJECT, 4).await;

    // when: its deletion is heard
    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    // then: the sweep carries on after the last scene of the batch
    let asked: MoreToSweepDTO =
        serde_json::from_value(wired.continuations()[0].payload.clone()).expect("readable");
    assert_eq!(asked.project, A_PROJECT);
    assert_eq!(
        asked.after,
        written[1].to_string(),
        "the cursor names the last scene of the batch just swept"
    );
}

#[tokio::test]
async fn another_project_keeps_its_prose() {
    // given: two projects holding scenes
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 2).await;
    let theirs = wired.holding(ANOTHER_PROJECT, 2).await;

    // when: one project's deletion is heard
    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    // then: the other keeps its scenes
    assert_eq!(
        wired.still_there(ANOTHER_PROJECT).await,
        theirs,
        "one deleted project must never take another author's book with it"
    );
}

#[tokio::test]
async fn hearing_the_same_deletion_twice_is_harmless() {
    // given: a project holding scenes
    let wired = a_workbench(4);
    wired.holding(A_PROJECT, 3).await;

    // when: its deletion is heard twice
    for _ in 0..2 {
        wired
            .sweep
            .handle(&deleted(A_PROJECT))
            .await
            .expect("a redelivery is not a failure");
    }

    // then: none of its scenes is left
    assert!(wired.still_there(A_PROJECT).await.is_empty());
}

#[tokio::test]
async fn a_deleted_project_takes_no_new_scene() {
    // given: a deleted project
    let wired = a_workbench(4);
    wired
        .sweep
        .handle(&deleted(A_PROJECT))
        .await
        .expect("sweeping succeeds");

    // when: a scene is created in it
    let refused = wired.scenes.create(A_PROJECT, &ada()).await;

    // then: it is refused as not there
    assert!(matches!(refused, Err(ScenesError::ProjectNotThere)));
    // and: nothing is written
    assert_eq!(
        wired.store.created.load(Ordering::Acquire),
        0,
        "a project known to be gone is not written to at all"
    );
    // and: another project still takes one
    assert!(
        wired.scenes.create(ANOTHER_PROJECT, &ada()).await.is_ok(),
        "another project's tombstone keeps nothing out"
    );
}

#[tokio::test]
async fn a_continuation_of_the_sweep_lays_no_tombstone() {
    // given: a project never deleted
    let wired = a_workbench(4);

    // when: a continuation of a sweep of it is heard
    wired
        .sweep
        .handle(&Message::opening(
            RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key is fine"),
            json!({ "project": A_PROJECT, "after": SceneId::generate(clock::at(500)).to_string() }),
            clock::at(1_000),
        ))
        .await
        .expect("sweeping succeeds");

    // then: the project still takes a new scene
    assert!(
        wired.scenes.create(A_PROJECT, &ada()).await.is_ok(),
        "only the deletion itself buries the project"
    );
}

#[tokio::test]
async fn a_message_about_no_project_at_all_is_refused() {
    // given: a message about no project at all
    let wired = a_workbench(4);
    let nonsense = Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        clock::at(1_000),
    );

    // then: hearing it fails
    assert!(
        wired.sweep.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}
