use std::sync::Arc;

use async_trait::async_trait;
use authorizing::AllowingEverything;
use belonging::InMemoryBelongings;
use clock::FixedClock;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_contract::{CREATED, DELETED, OPENING_CHANGED, RETITLED};
use scenes_core::{
    CatalogError, ProjectLink, SceneCatalog, SceneChange, SceneId, SceneSummary, SceneTitle,
    ScenesCore,
};
use scenes_messaging::CatalogueScenesListener;
use scenes_store::InMemorySceneStore;
use serde_json::json;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};

use crate::committed::Committed;

fn a_project() -> ProjectLink {
    ProjectLink::from("project_1")
}

fn a_title(saying: &str) -> SceneTitle {
    SceneTitle::new(saying).expect("a plain title is fine")
}

struct Wired {
    catalog: Arc<InMemorySceneCatalog>,
    catalogue: Inboxed<CatalogueScenesListener>,
}

fn a_listener_cataloguing_in(catalog: Arc<dyn SceneCatalog>) -> Inboxed<CatalogueScenesListener> {
    let transactions = Arc::new(InMemoryTransactions::new());
    let clock = Arc::new(FixedClock::new(clock::at(1_000)));

    Inboxed::new(
        CatalogueScenesListener::new(ScenesCore::new(
            Arc::new(InMemorySceneStore::new()),
            transactions.clone(),
            catalog,
            Arc::new(InMemoryQuotationRestylingTasks::new()),
            Arc::new(InMemoryBelongings::new()),
            Arc::new(AllowingEverything),
            clock.clone(),
        )),
        transactions,
        Arc::new(InMemoryInbox::new(clock)),
    )
}

fn a_workbench() -> Wired {
    let catalog = Arc::new(InMemorySceneCatalog::new());

    Wired {
        catalogue: a_listener_cataloguing_in(catalog.clone()),
        catalog,
    }
}

struct OutOfReach;

fn out_of_reach() -> CatalogError {
    CatalogError::Backend("the catalog is down".into())
}

#[async_trait]
impl SceneCatalog for OutOfReach {
    async fn remember(
        &self,
        _transaction: &mut Transaction,
        _id: SceneId,
        _project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        Err(out_of_reach())
    }

    async fn retitle(
        &self,
        _transaction: &mut Transaction,
        _id: SceneId,
        _project: &ProjectLink,
        _title: &SceneTitle,
        _version: u64,
    ) -> Result<(), CatalogError> {
        Err(out_of_reach())
    }

    async fn note_opening(
        &self,
        _transaction: &mut Transaction,
        _id: SceneId,
        _project: &ProjectLink,
        _opening: &str,
        _at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        Err(out_of_reach())
    }

    async fn forget(
        &self,
        _transaction: &mut Transaction,
        _id: SceneId,
        _at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        Err(out_of_reach())
    }

    async fn forget_tombstones_laid_before(
        &self,
        _transaction: &mut Transaction,
        _before: OffsetDateTime,
        _at_most: u64,
    ) -> Result<u64, CatalogError> {
        Err(out_of_reach())
    }

    async fn summary_of(&self, _id: SceneId) -> Result<Option<SceneSummary>, CatalogError> {
        Err(out_of_reach())
    }

    async fn in_project_after(
        &self,
        _project: &ProjectLink,
        _after: Option<SceneId>,
        _at_most: usize,
    ) -> Result<Vec<SceneSummary>, CatalogError> {
        Err(out_of_reach())
    }

    async fn lay_project_tombstone(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        Err(out_of_reach())
    }

    async fn has_project_tombstone(&self, _project: &ProjectLink) -> Result<bool, CatalogError> {
        Err(out_of_reach())
    }

    async fn has_project_tombstone_within(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
    ) -> Result<bool, CatalogError> {
        Err(out_of_reach())
    }
}

impl Wired {
    async fn hears(&self, change: SceneChange, seconds: i64) {
        self.catalogue
            .handle(&scenes_messaging::message_for(&change, clock::at(seconds)))
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
    SceneId::generate(clock::at(1_000))
}

#[test]
fn it_listens_for_what_names_a_scene_and_for_deletions() {
    // when: what it listens to is asked
    let keys: Vec<String> = a_workbench()
        .catalogue
        .listens_to()
        .iter()
        .map(ToString::to_string)
        .collect();

    // then: it is creations, retitlings, openings and deletions
    assert_eq!(keys, vec![CREATED, RETITLED, OPENING_CHANGED, DELETED]);
}

#[tokio::test]
async fn a_created_scene_is_catalogued_under_its_project() {
    // given: an empty catalog
    let bench = a_workbench();
    let scene = a_scene();

    // when: a scene's creation is heard
    bench.hears(created(scene), 1_000).await;

    // then: it is listed under its project
    assert_eq!(bench.listed().await, vec![summary(scene, "", "")]);
}

#[tokio::test]
async fn a_title_heard_again_at_the_version_the_catalog_holds_changes_nothing() {
    // given: a catalogued scene titled at some version
    let bench = a_workbench();
    let scene = a_scene();
    bench.hears(created(scene), 1_000).await;
    bench
        .catalog
        .retitle_committed(scene, &a_project(), &a_title("The loom"), 2)
        .await
        .expect("retitling should succeed");

    // when: another title at that version is heard
    bench.hears(retitled(scene, "Not the loom", 2), 2_000).await;

    // then: the title stays
    assert_eq!(
        bench.listed().await,
        vec![summary(scene, "The loom", "")],
        "the version the message carries is the one the catalog compares"
    );
}

#[tokio::test]
async fn a_catalog_out_of_reach_fails_the_listener_so_the_message_is_tried_again() {
    // given: a catalog out of reach
    let catalogue = a_listener_cataloguing_in(Arc::new(OutOfReach));
    let scene = a_scene();

    // then: hearing any change of a scene fails
    for change in [
        created(scene),
        retitled(scene, "The loom", 2),
        opened(scene, "The loom stood silent."),
        SceneChange::Deleted {
            scene,
            project: a_project(),
        },
    ] {
        assert!(
            catalogue
                .handle(&scenes_messaging::message_for(&change, clock::at(1_000)))
                .await
                .is_err(),
            "{change:?} was taken as done though the catalog never heard it"
        );
    }
}

#[tokio::test]
async fn a_scene_is_listed_with_its_newest_title_and_opening() {
    // given: a catalogued scene
    let bench = a_workbench();
    let scene = a_scene();
    bench.hears(created(scene), 1_000).await;

    // when: a newer title is heard before an older one
    bench
        .hears(retitled(scene, "The loom, again", 3), 3_000)
        .await;
    bench.hears(retitled(scene, "The loom", 2), 4_000).await;
    // and: a newer opening before an older one
    bench.hears(opened(scene, "Then it spoke."), 3_000).await;
    bench
        .hears(opened(scene, "The loom stood silent."), 2_000)
        .await;

    // then: it is listed with the newer of each
    assert_eq!(
        bench.listed().await,
        vec![summary(scene, "The loom, again", "Then it spoke.")],
        "a title is ordered by the scene's version and an opening by when it was announced, not by when either arrived"
    );
}

#[tokio::test]
async fn a_deleted_scene_stays_out_of_the_catalog() {
    // given: a catalogued scene
    let bench = a_workbench();
    let scene = a_scene();
    bench.hears(created(scene), 1_000).await;

    // when: its deletion is heard
    bench
        .hears(
            SceneChange::Deleted {
                scene,
                project: a_project(),
            },
            2_000,
        )
        .await;
    // and: a later title of it
    bench.hears(retitled(scene, "The loom", 9), 3_000).await;

    // then: it is not listed
    assert!(bench.listed().await.is_empty());
}

#[tokio::test]
async fn a_message_naming_no_scene_is_refused() {
    // given: an empty catalog
    let bench = a_workbench();

    // when: a message naming no scene is heard
    let refused = bench
        .catalogue
        .handle(&Message::opening(
            RoutingKey::parse(CREATED).expect("a declared key"),
            json!({ "scene": "idea_1", "project": "project_1" }),
            clock::at(1_000),
        ))
        .await;

    // then: it is refused
    assert!(
        refused.is_err(),
        "a listener that shrugs off a broken message hides it, so it is kept for a look instead"
    );
    // and: nothing is listed
    assert!(bench.listed().await.is_empty());
}

#[tokio::test]
async fn a_deleted_scene_s_tombstone_is_laid_when_it_was_deleted() {
    // given: a scene's deletion heard
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
    let scenes = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        bench.catalog.clone(),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_000))),
    );

    // then: its tombstone was laid at the deletion and is forgotten once
    for (before, forgotten) in [
        (clock::at(5_000), 0),
        (clock::at(5_001), 1),
        (clock::at(5_001), 0),
    ] {
        assert_eq!(
            scenes
                .forget_tombstones_laid_before(before, u64::MAX)
                .await
                .expect("forgetting tombstones should succeed"),
            forgotten,
            "laid when it happened, and forgotten for good"
        );
    }
}
