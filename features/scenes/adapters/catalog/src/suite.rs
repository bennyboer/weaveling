use scenes_core::{ProjectLink, SceneCatalog, SceneId, SceneSummary, SceneTitle};
use time::{Duration, OffsetDateTime};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: SceneCatalog;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    async fn cleanup(self);
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_scene(made_at: i64) -> SceneId {
    SceneId::generate(at(made_at))
}

fn a_project(named: &str) -> ProjectLink {
    ProjectLink::from(named)
}

fn a_title(saying: &str) -> SceneTitle {
    SceneTitle::new(saying).expect("a plain title is fine")
}

fn summary(id: SceneId, title: &str, opening: &str) -> SceneSummary {
    SceneSummary {
        id,
        project: a_project("project_1"),
        title: a_title(title),
        opening: opening.to_owned(),
    }
}

async fn remembered(catalog: &impl SceneCatalog, id: SceneId) {
    catalog
        .remember(id, &a_project("project_1"))
        .await
        .expect("remembering should succeed");
}

async fn retitled(catalog: &impl SceneCatalog, id: SceneId, title: &str, version: u64) {
    catalog
        .retitle(id, &a_project("project_1"), &a_title(title), version)
        .await
        .expect("retitling should succeed");
}

async fn opened(catalog: &impl SceneCatalog, id: SceneId, opening: &str, seconds: i64) {
    catalog
        .note_opening(id, &a_project("project_1"), opening, at(seconds))
        .await
        .expect("noting an opening should succeed");
}

async fn forgotten(catalog: &impl SceneCatalog, id: SceneId) {
    catalog.forget(id).await.expect("forgetting should succeed");
}

async fn listed(catalog: &impl SceneCatalog, project: &str) -> Vec<SceneSummary> {
    catalog
        .in_project_after(&a_project(project), None, 100)
        .await
        .expect("listing should succeed")
}

pub async fn a_project_nobody_wrote_in_lists_nothing(catalog: &impl SceneCatalog) {
    assert!(listed(catalog, "project_1").await.is_empty());
}

pub async fn a_remembered_scene_is_listed_untitled_and_without_an_opening(
    catalog: &impl SceneCatalog,
) {
    let id = a_scene(1_000);

    remembered(catalog, id).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "", "")]
    );
}

pub async fn another_project_sees_none_of_them(catalog: &impl SceneCatalog) {
    remembered(catalog, a_scene(1_000)).await;

    assert!(listed(catalog, "project_2").await.is_empty());
}

pub async fn scenes_are_listed_in_order_a_page_at_a_time(catalog: &impl SceneCatalog) {
    let [first, second, third] = [a_scene(1_000), a_scene(2_000), a_scene(3_000)];
    for id in [third, first, second] {
        remembered(catalog, id).await;
    }

    let page = catalog
        .in_project_after(&a_project("project_1"), None, 2)
        .await
        .expect("listing should succeed");
    let rest = catalog
        .in_project_after(&a_project("project_1"), Some(second), 2)
        .await
        .expect("listing should succeed");

    assert_eq!(
        page.iter().map(|scene| scene.id).collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(
        rest.iter().map(|scene| scene.id).collect::<Vec<_>>(),
        vec![third],
        "the next page starts after the last scene of the one before"
    );
}

pub async fn a_retitle_names_the_scene(catalog: &impl SceneCatalog) {
    let id = a_scene(1_000);
    remembered(catalog, id).await;

    retitled(catalog, id, "The loom", 1).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "The loom", "")]
    );
}

pub async fn a_late_retitle_does_not_undo_a_newer_one(catalog: &impl SceneCatalog) {
    let id = a_scene(1_000);
    remembered(catalog, id).await;

    retitled(catalog, id, "The loom, again", 2).await;
    retitled(catalog, id, "The loom", 1).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "The loom, again", "")],
        "the version tells which title came last, whatever order they arrive in"
    );
}

pub async fn an_opening_is_kept_beside_the_title(catalog: &impl SceneCatalog) {
    let id = a_scene(1_000);
    remembered(catalog, id).await;
    retitled(catalog, id, "The loom", 1).await;

    opened(catalog, id, "The loom stood silent.", 1_000).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "The loom", "The loom stood silent.")]
    );
}

pub async fn a_late_opening_does_not_undo_a_newer_one(catalog: &impl SceneCatalog) {
    let id = a_scene(1_000);
    remembered(catalog, id).await;

    opened(catalog, id, "Then it spoke.", 2_000).await;
    opened(catalog, id, "The loom stood silent.", 1_000).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "", "Then it spoke.")]
    );
}

pub async fn a_scene_named_before_it_was_remembered_is_listed_and_keeps_its_name(
    catalog: &impl SceneCatalog,
) {
    let id = a_scene(1_000);

    retitled(catalog, id, "The loom", 1).await;
    opened(catalog, id, "The loom stood silent.", 1_000).await;
    remembered(catalog, id).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "The loom", "The loom stood silent.")],
        "messages may come in any order, and the one saying a scene exists must not wipe its name"
    );
}

pub async fn a_forgotten_scene_is_not_listed(catalog: &impl SceneCatalog) {
    let [gone, kept] = [a_scene(1_000), a_scene(2_000)];
    remembered(catalog, gone).await;
    remembered(catalog, kept).await;

    forgotten(catalog, gone).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(kept, "", "")]
    );
}

pub async fn nothing_arriving_after_a_scene_was_forgotten_brings_it_back(
    catalog: &impl SceneCatalog,
) {
    let id = a_scene(1_000);
    forgotten(catalog, id).await;

    remembered(catalog, id).await;
    retitled(catalog, id, "The loom", 7).await;
    opened(catalog, id, "The loom stood silent.", 9_000).await;

    assert!(
        listed(catalog, "project_1").await.is_empty(),
        "a scene named just before it was deleted can have its name arrive after the deletion"
    );
}

pub async fn forgetting_what_was_never_remembered_is_harmless(catalog: &impl SceneCatalog) {
    let id = a_scene(1_000);

    forgotten(catalog, id).await;
    forgotten(catalog, id).await;

    assert!(listed(catalog, "project_1").await.is_empty());
}

macro_rules! conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.catalog()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::suite::conformance_case!($workbench, a_project_nobody_wrote_in_lists_nothing);
        $crate::suite::conformance_case!(
            $workbench,
            a_remembered_scene_is_listed_untitled_and_without_an_opening
        );
        $crate::suite::conformance_case!($workbench, another_project_sees_none_of_them);
        $crate::suite::conformance_case!($workbench, scenes_are_listed_in_order_a_page_at_a_time);
        $crate::suite::conformance_case!($workbench, a_retitle_names_the_scene);
        $crate::suite::conformance_case!($workbench, a_late_retitle_does_not_undo_a_newer_one);
        $crate::suite::conformance_case!($workbench, an_opening_is_kept_beside_the_title);
        $crate::suite::conformance_case!($workbench, a_late_opening_does_not_undo_a_newer_one);
        $crate::suite::conformance_case!(
            $workbench,
            a_scene_named_before_it_was_remembered_is_listed_and_keeps_its_name
        );
        $crate::suite::conformance_case!($workbench, a_forgotten_scene_is_not_listed);
        $crate::suite::conformance_case!(
            $workbench,
            nothing_arriving_after_a_scene_was_forgotten_brings_it_back
        );
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_what_was_never_remembered_is_harmless
        );
    };
}

pub(crate) use {conformance_case, conformance_tests};
