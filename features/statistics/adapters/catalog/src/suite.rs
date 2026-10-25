use statistics_core::{ProjectLink, SceneLink, WordCount, WordCountCatalog};
use time::{Duration, OffsetDateTime};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: WordCountCatalog;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    async fn cleanup(self);
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_scene(named: &str) -> SceneLink {
    SceneLink::from(named)
}

fn a_project(named: &str) -> ProjectLink {
    ProjectLink::from(named)
}

fn counted(scene: &str, words: u64) -> WordCount {
    WordCount {
        scene: a_scene(scene),
        words,
    }
}

async fn kept(catalog: &impl WordCountCatalog, scene: &str, words: u64, seconds: i64) {
    catalog
        .keep(&a_scene(scene), &a_project("project_1"), words, at(seconds))
        .await
        .expect("keeping should succeed");
}

async fn in_project(catalog: &impl WordCountCatalog, project: &str) -> Vec<WordCount> {
    catalog
        .in_project(&a_project(project))
        .await
        .expect("asking should succeed")
}

async fn forgotten(catalog: &impl WordCountCatalog, scene: &str) {
    forgotten_at(catalog, scene, 10_000).await;
}

async fn forgotten_at(catalog: &impl WordCountCatalog, scene: &str, seconds: i64) {
    catalog
        .forget_scene(&a_scene(scene), at(seconds))
        .await
        .expect("forgetting should succeed");
}

pub async fn a_project_nobody_counted_has_no_counts(catalog: &impl WordCountCatalog) {
    assert!(in_project(catalog, "project_1").await.is_empty());
}

pub async fn a_kept_count_comes_back_with_its_project(catalog: &impl WordCountCatalog) {
    kept(catalog, "scene_1", 42, 1_000).await;

    assert_eq!(
        in_project(catalog, "project_1").await,
        vec![counted("scene_1", 42)]
    );
}

pub async fn another_project_sees_none_of_them(catalog: &impl WordCountCatalog) {
    kept(catalog, "scene_1", 42, 1_000).await;

    assert!(in_project(catalog, "project_2").await.is_empty());
}

pub async fn counts_come_back_in_one_order_whatever_order_they_were_kept_in(
    catalog: &impl WordCountCatalog,
) {
    kept(catalog, "scene_2", 2, 1_000).await;
    kept(catalog, "scene_3", 3, 1_000).await;
    kept(catalog, "scene_1", 1, 1_000).await;

    assert_eq!(
        in_project(catalog, "project_1").await,
        vec![
            counted("scene_1", 1),
            counted("scene_2", 2),
            counted("scene_3", 3)
        ]
    );
}

pub async fn a_newer_count_takes_the_place_of_an_older_one(catalog: &impl WordCountCatalog) {
    kept(catalog, "scene_1", 42, 1_000).await;
    kept(catalog, "scene_1", 50, 2_000).await;

    assert_eq!(
        in_project(catalog, "project_1").await,
        vec![counted("scene_1", 50)]
    );
}

pub async fn a_late_count_does_not_undo_a_newer_one(catalog: &impl WordCountCatalog) {
    kept(catalog, "scene_1", 50, 2_000).await;
    kept(catalog, "scene_1", 42, 1_000).await;

    assert_eq!(
        in_project(catalog, "project_1").await,
        vec![counted("scene_1", 50)],
        "counts may arrive out of order, and the one counted last is the one that is true"
    );
}

pub async fn a_forgotten_scene_has_no_count(catalog: &impl WordCountCatalog) {
    kept(catalog, "scene_1", 42, 1_000).await;
    kept(catalog, "scene_2", 7, 1_000).await;

    forgotten(catalog, "scene_1").await;

    assert_eq!(
        in_project(catalog, "project_1").await,
        vec![counted("scene_2", 7)]
    );
}

pub async fn a_count_arriving_after_its_scene_was_forgotten_does_not_bring_it_back(
    catalog: &impl WordCountCatalog,
) {
    forgotten(catalog, "scene_1").await;

    kept(catalog, "scene_1", 42, 3_000).await;

    assert!(
        in_project(catalog, "project_1").await.is_empty(),
        "a scene counted just before it was deleted can have its count arrive after the deletion"
    );
}

pub async fn keeping_says_what_is_kept(catalog: &impl WordCountCatalog) {
    let keeping = |words: u64, seconds: i64| async move {
        catalog
            .keep(
                &a_scene("scene_1"),
                &a_project("project_1"),
                words,
                at(seconds),
            )
            .await
            .expect("keeping should succeed")
    };

    let first = keeping(42, 1_000).await;
    let newer = keeping(50, 2_000).await;
    let late = keeping(7, 1_500).await;
    forgotten(catalog, "scene_1").await;
    let gone = keeping(60, 3_000).await;

    assert_eq!(
        [first, newer, late, gone],
        [Some(42), Some(50), Some(50), None],
        "a late count leaves the newer one kept, and a forgotten scene keeps none"
    );
}

pub async fn forgetting_what_was_never_counted_is_harmless(catalog: &impl WordCountCatalog) {
    forgotten(catalog, "scene_1").await;
    forgotten(catalog, "scene_1").await;

    assert!(in_project(catalog, "project_1").await.is_empty());
}

async fn forget_long_gone(
    catalog: &impl WordCountCatalog,
    before_seconds: i64,
    at_most: u64,
) -> u64 {
    catalog
        .forget_tombstones_laid_before(at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

pub async fn a_forgotten_tombstone_keeps_nothing_out_and_a_later_one_still_does(
    catalog: &impl WordCountCatalog,
) {
    forgotten_at(catalog, "scene_long_gone", 1_000).await;
    forgotten_at(catalog, "scene_just_gone", 2_000).await;

    assert_eq!(forget_long_gone(catalog, 2_000, u64::MAX).await, 1);

    kept(catalog, "scene_long_gone", 5, 3_000).await;
    kept(catalog, "scene_just_gone", 6, 3_000).await;
    assert_eq!(
        in_project(catalog, "project_1").await,
        vec![counted("scene_long_gone", 5)],
        "laid at the moment asked about is not before it"
    );
}

pub async fn a_scene_s_tombstone_is_laid_once_when_it_first_went(catalog: &impl WordCountCatalog) {
    forgotten_at(catalog, "scene_1", 1_000).await;
    forgotten_at(catalog, "scene_1", 5_000).await;

    assert_eq!(
        forget_long_gone(catalog, 2_000, u64::MAX).await,
        1,
        "a deletion heard again is the same deletion"
    );
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &impl WordCountCatalog,
) {
    for (scene, seconds) in [
        ("scene_newest", 1_002),
        ("scene_oldest", 1_000),
        ("scene_older", 1_001),
    ] {
        forgotten_at(catalog, scene, seconds).await;
    }

    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 2);

    for scene in ["scene_newest", "scene_oldest", "scene_older"] {
        kept(catalog, scene, 7, 9_500).await;
    }
    assert_eq!(
        in_project(catalog, "project_1").await,
        vec![counted("scene_older", 7), counted("scene_oldest", 7)]
    );
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 1);
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
        $crate::suite::conformance_case!($workbench, a_project_nobody_counted_has_no_counts);
        $crate::suite::conformance_case!($workbench, a_kept_count_comes_back_with_its_project);
        $crate::suite::conformance_case!($workbench, another_project_sees_none_of_them);
        $crate::suite::conformance_case!(
            $workbench,
            counts_come_back_in_one_order_whatever_order_they_were_kept_in
        );
        $crate::suite::conformance_case!($workbench, a_newer_count_takes_the_place_of_an_older_one);
        $crate::suite::conformance_case!($workbench, a_late_count_does_not_undo_a_newer_one);
        $crate::suite::conformance_case!($workbench, a_forgotten_scene_has_no_count);
        $crate::suite::conformance_case!(
            $workbench,
            a_count_arriving_after_its_scene_was_forgotten_does_not_bring_it_back
        );
        $crate::suite::conformance_case!($workbench, keeping_says_what_is_kept);
        $crate::suite::conformance_case!($workbench, forgetting_what_was_never_counted_is_harmless);
        $crate::suite::conformance_case!(
            $workbench,
            a_forgotten_tombstone_keeps_nothing_out_and_a_later_one_still_does
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_scene_s_tombstone_is_laid_once_when_it_first_went
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
    };
}

pub(crate) use {conformance_case, conformance_tests};
