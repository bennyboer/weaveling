use std::sync::Arc;

use clock::FixedClock;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use scenes_contract::{DELETED, SceneDeletedDTO, WORDS_COUNTED, WordsCountedDTO};
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::{ProjectLink, SceneLink, WordCount, WordCountCatalog};
use statistics_messaging::ProjectWordCountsListener;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transactions};

struct Wired {
    catalog: Arc<InMemoryWordCountCatalog>,
    projector: Inboxed<ProjectWordCountsListener>,
}

fn a_workbench() -> Wired {
    a_workbench_hearing_in(InMemoryTransactions::new())
}

fn a_workbench_hearing_in(hearings: InMemoryTransactions) -> Wired {
    let catalog = Arc::new(InMemoryWordCountCatalog::new());

    Wired {
        projector: Inboxed::new(
            ProjectWordCountsListener::new(crate::allowing::everyone(
                Arc::new(InMemoryTransactions::new()),
                catalog.clone(),
            )),
            Arc::new(hearings),
            Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                OffsetDateTime::UNIX_EPOCH,
            )))),
        ),
        catalog,
    }
}

impl Wired {
    async fn hears(&self, message: Message) {
        self.projector
            .handle(&message)
            .await
            .expect("the listener should take this message");
    }

    async fn counts_in(&self, project: &str) -> Vec<WordCount> {
        self.catalog
            .in_project(&ProjectLink::from(project))
            .await
            .expect("asking should succeed")
    }
}

fn counted(scene: &str, words: u64, seconds: i64) -> Message {
    Message::opening(
        RoutingKey::parse(WORDS_COUNTED).expect("a declared key"),
        serde_json::to_value(WordsCountedDTO {
            scene: scene.to_owned(),
            project: "project_1".to_owned(),
            words,
        })
        .expect("a count is plain data"),
        clock::at(seconds),
    )
}

fn deleted(scene: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared key"),
        serde_json::to_value(SceneDeletedDTO {
            scene: scene.to_owned(),
        })
        .expect("a deletion is plain data"),
        clock::at(5_000),
    )
}

fn a_count(scene: &str, words: u64) -> WordCount {
    WordCount {
        scene: SceneLink::from(scene),
        words,
    }
}

#[test]
fn it_listens_for_counts_and_for_deletions() {
    // when: what it listens to is asked
    let keys: Vec<String> = a_workbench()
        .projector
        .listens_to()
        .iter()
        .map(ToString::to_string)
        .collect();

    // then: it is counts and deletions
    assert_eq!(keys, vec![WORDS_COUNTED, DELETED]);
}

#[tokio::test]
async fn a_counted_scene_is_kept_under_its_project() {
    // given: nothing counted yet
    let bench = a_workbench();

    // when: a count of a scene is heard
    bench.hears(counted("scene_1", 42, 1_000)).await;

    // then: its project has it
    assert_eq!(
        bench.counts_in("project_1").await,
        vec![a_count("scene_1", 42)]
    );
}

#[tokio::test]
async fn a_count_is_ordered_by_when_it_was_counted() {
    // given: a count of a scene heard
    let bench = a_workbench();
    bench.hears(counted("scene_1", 50, 2_000)).await;

    // when: an older count of it is heard
    bench.hears(counted("scene_1", 42, 1_000)).await;

    // then: the newer one stands
    assert_eq!(
        bench.counts_in("project_1").await,
        vec![a_count("scene_1", 50)],
        "a redelivered older count must not stand over the newer one"
    );
}

#[tokio::test]
async fn a_deleted_scene_is_forgotten_for_good() {
    // given: a counted scene
    let bench = a_workbench();
    bench.hears(counted("scene_1", 42, 1_000)).await;

    // when: its deletion is heard
    bench.hears(deleted("scene_1")).await;
    // and: a later count of it
    bench.hears(counted("scene_1", 43, 6_000)).await;

    // then: its project has no count
    assert!(bench.counts_in("project_1").await.is_empty());
}

#[tokio::test]
async fn a_deleted_scene_s_tombstone_is_laid_when_it_was_deleted() {
    // given: a deletion heard
    let bench = a_workbench();
    bench.hears(deleted("scene_1")).await;
    let statistics =
        crate::allowing::everyone(Arc::new(InMemoryTransactions::new()), bench.catalog.clone());

    // then: no tombstone was laid before the deletion
    assert_eq!(
        statistics
            .forget_tombstones_laid_before(clock::at(5_000), u64::MAX)
            .await
            .expect("forgetting tombstones should succeed"),
        0,
        "laid when it happened, not when it was heard"
    );
    // and: one was laid at it
    assert_eq!(
        statistics
            .forget_tombstones_laid_before(clock::at(5_001), u64::MAX)
            .await
            .expect("forgetting tombstones should succeed"),
        1
    );
    // and: once forgotten it is gone
    assert_eq!(
        statistics
            .forget_tombstones_laid_before(clock::at(5_001), u64::MAX)
            .await
            .expect("forgetting tombstones should succeed"),
        0,
        "forgotten for good"
    );
}

#[tokio::test]
async fn a_deletion_whose_hearing_does_not_commit_is_undone_with_it() {
    // given: hearings that never commit
    let bench = a_workbench_hearing_in(InMemoryTransactions::refusing_commits());
    // and: a kept count
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("an in-memory transaction begins");
    bench
        .catalog
        .keep(
            &mut transaction,
            &SceneLink::from("scene_1"),
            &ProjectLink::from("project_1"),
            42,
            clock::at(1_000),
        )
        .await
        .expect("keeping should succeed");
    transaction
        .finish()
        .await
        .expect("an in-memory transaction commits");

    // when: the scene's deletion is heard
    bench
        .projector
        .handle(&deleted("scene_1"))
        .await
        .expect_err("a hearing that does not commit is not handled");

    // then: the count is still kept
    assert_eq!(
        bench.counts_in("project_1").await,
        vec![a_count("scene_1", 42)],
        "the deletion is written in the hearing's transaction"
    );
}
