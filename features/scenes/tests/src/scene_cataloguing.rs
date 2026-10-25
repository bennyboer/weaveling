use std::sync::Arc;

use messaging::{Listener, Message, RoutingKey};
use scenes_catalog::InMemorySceneCatalog;
use scenes_contract::{CREATED, DELETED, OPENING_CHANGED, RETITLED};
use scenes_core::{ProjectLink, SceneCatalog, SceneChange, SceneId, SceneSummary, SceneTitle};
use scenes_messaging::CatalogueScenesListener;
use serde_json::json;
use time::{Duration, OffsetDateTime};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_project() -> ProjectLink {
    ProjectLink::from("project_1")
}

fn a_title(saying: &str) -> SceneTitle {
    SceneTitle::new(saying).expect("a plain title is fine")
}

struct Wired {
    catalog: Arc<InMemorySceneCatalog>,
    catalogue: CatalogueScenesListener,
}

fn a_workbench() -> Wired {
    let catalog = Arc::new(InMemorySceneCatalog::new());

    Wired {
        catalogue: CatalogueScenesListener::new(catalog.clone()),
        catalog,
    }
}

impl Wired {
    async fn hears(&self, change: SceneChange, seconds: i64) {
        self.catalogue
            .handle(&scenes_messaging::message_for(&change, at(seconds)))
            .await
            .expect("the listener should take what scenes announces");
    }

    async fn listed(&self) -> Vec<SceneSummary> {
        self.catalog
            .in_project_after(&a_project(), None, 100)
            .await
            .expect("listing should succeed")
    }
}

fn created(scene: SceneId) -> SceneChange {
    SceneChange::Created {
        scene,
        project: a_project(),
    }
}

fn retitled(scene: SceneId, title: &str, version: u64) -> SceneChange {
    SceneChange::Retitled {
        scene,
        project: a_project(),
        title: a_title(title),
        version,
    }
}

fn opened(scene: SceneId, opening: &str) -> SceneChange {
    SceneChange::OpeningChanged {
        scene,
        project: a_project(),
        opening: opening.to_owned(),
    }
}

fn summary(id: SceneId, title: &str, opening: &str) -> SceneSummary {
    SceneSummary {
        id,
        project: a_project(),
        title: a_title(title),
        opening: opening.to_owned(),
    }
}

fn a_scene() -> SceneId {
    SceneId::generate(at(1_000))
}

#[test]
fn it_listens_for_what_names_a_scene_and_for_deletions() {
    let keys: Vec<String> = a_workbench()
        .catalogue
        .listens_to()
        .iter()
        .map(ToString::to_string)
        .collect();

    assert_eq!(keys, vec![CREATED, RETITLED, OPENING_CHANGED, DELETED]);
}

#[tokio::test]
async fn a_created_scene_is_catalogued_under_its_project() {
    let bench = a_workbench();
    let scene = a_scene();

    bench.hears(created(scene), 1_000).await;

    assert_eq!(bench.listed().await, vec![summary(scene, "", "")]);
}

#[tokio::test]
async fn a_scene_is_listed_with_its_newest_title_and_opening() {
    let bench = a_workbench();
    let scene = a_scene();
    bench.hears(created(scene), 1_000).await;

    bench
        .hears(retitled(scene, "The loom, again", 3), 3_000)
        .await;
    bench.hears(retitled(scene, "The loom", 2), 4_000).await;
    bench.hears(opened(scene, "Then it spoke."), 3_000).await;
    bench
        .hears(opened(scene, "The loom stood silent."), 2_000)
        .await;

    assert_eq!(
        bench.listed().await,
        vec![summary(scene, "The loom, again", "Then it spoke.")],
        "a title is ordered by the scene's version and an opening by when it was announced, not by when either arrived"
    );
}

#[tokio::test]
async fn a_deleted_scene_stays_out_of_the_catalog() {
    let bench = a_workbench();
    let scene = a_scene();
    bench.hears(created(scene), 1_000).await;

    bench
        .hears(
            SceneChange::Deleted {
                scene,
                project: a_project(),
            },
            2_000,
        )
        .await;
    bench.hears(retitled(scene, "The loom", 9), 3_000).await;

    assert!(bench.listed().await.is_empty());
}

#[tokio::test]
async fn a_message_naming_no_scene_is_refused() {
    let bench = a_workbench();

    let refused = bench
        .catalogue
        .handle(&Message::opening(
            RoutingKey::parse(CREATED).expect("a declared key"),
            json!({ "scene": "idea_1", "project": "project_1" }),
            at(1_000),
        ))
        .await;

    assert!(
        refused.is_err(),
        "a listener that shrugs off a broken message hides it, so it is kept for a look instead"
    );
    assert!(bench.listed().await.is_empty());
}

#[tokio::test]
async fn a_deleted_scene_s_tombstone_is_laid_when_it_was_deleted() {
    let bench = a_workbench();
    let scene = a_scene();
    bench
        .hears(
            SceneChange::Deleted {
                scene,
                project: a_project(),
            },
            5_000,
        )
        .await;

    for (before, forgotten) in [(at(5_000), 0), (at(5_001), 1)] {
        assert_eq!(
            bench
                .catalog
                .forget_tombstones_laid_before(before, u64::MAX)
                .await
                .expect("forgetting tombstones should succeed"),
            forgotten,
            "laid when it happened"
        );
    }
}
