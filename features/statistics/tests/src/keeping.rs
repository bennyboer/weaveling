use std::sync::Arc;

use authorizing::AllowingEverything;
use messaging::{Listener, Message, RoutingKey};
use scenes_contract::{DELETED, SceneDeletedDTO, WORDS_COUNTED, WordsCountedDTO};
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::{ProjectLink, SceneLink, StatisticsCore, WordCount, WordCountCatalog};
use statistics_messaging::ProjectWordCountsListener;
use time::{Duration, OffsetDateTime};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Wired {
    catalog: Arc<InMemoryWordCountCatalog>,
    projector: ProjectWordCountsListener,
}

fn a_workbench() -> Wired {
    let catalog = Arc::new(InMemoryWordCountCatalog::new());

    Wired {
        projector: ProjectWordCountsListener::new(StatisticsCore::new(
            catalog.clone(),
            Arc::new(AllowingEverything),
        )),
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

    async fn in_project(&self, project: &str) -> Vec<WordCount> {
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
        at(seconds),
    )
}

fn deleted(scene: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared key"),
        serde_json::to_value(SceneDeletedDTO {
            scene: scene.to_owned(),
        })
        .expect("a deletion is plain data"),
        at(5_000),
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
    let keys: Vec<String> = a_workbench()
        .projector
        .listens_to()
        .iter()
        .map(ToString::to_string)
        .collect();

    assert_eq!(keys, vec![WORDS_COUNTED, DELETED]);
}

#[tokio::test]
async fn a_counted_scene_is_kept_under_its_project() {
    let bench = a_workbench();

    bench.hears(counted("scene_1", 42, 1_000)).await;

    assert_eq!(
        bench.in_project("project_1").await,
        vec![a_count("scene_1", 42)]
    );
}

#[tokio::test]
async fn a_count_is_ordered_by_when_it_was_counted() {
    let bench = a_workbench();

    bench.hears(counted("scene_1", 50, 2_000)).await;
    bench.hears(counted("scene_1", 42, 1_000)).await;

    assert_eq!(
        bench.in_project("project_1").await,
        vec![a_count("scene_1", 50)],
        "a redelivered older count must not stand over the newer one"
    );
}

#[tokio::test]
async fn a_deleted_scene_is_forgotten_for_good() {
    let bench = a_workbench();
    bench.hears(counted("scene_1", 42, 1_000)).await;

    bench.hears(deleted("scene_1")).await;
    bench.hears(counted("scene_1", 43, 6_000)).await;

    assert!(bench.in_project("project_1").await.is_empty());
}

#[tokio::test]
async fn a_deleted_scene_s_tombstone_is_laid_when_it_was_deleted() {
    let bench = a_workbench();
    bench.hears(deleted("scene_1")).await;

    let forgotten_before = |moment| {
        bench
            .catalog
            .forget_tombstones_laid_before(moment, u64::MAX)
    };

    assert_eq!(
        forgotten_before(at(5_000))
            .await
            .expect("forgetting tombstones should succeed"),
        0,
        "laid when it happened, not when it was heard"
    );
    assert_eq!(
        forgotten_before(at(5_001))
            .await
            .expect("forgetting tombstones should succeed"),
        1
    );
}
