use scenes_core::{
    ProjectLink, QuotationRestylingProgress, QuotationRestylingTasks, SceneCatalog, SceneId,
    SceneSummary, SceneTitle,
};
use time::{Duration, OffsetDateTime};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: SceneCatalog;
    type Tasks: QuotationRestylingTasks;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    fn tasks(&self) -> &Self::Tasks;

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
    forgotten_at(catalog, id, 10_000).await;
}

async fn forgotten_at(catalog: &impl SceneCatalog, id: SceneId, seconds: i64) {
    catalog
        .forget(id, at(seconds))
        .await
        .expect("forgetting should succeed");
}

async fn forget_long_gone(catalog: &impl SceneCatalog, before_seconds: i64, at_most: u64) -> u64 {
    catalog
        .forget_tombstones_laid_before(at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
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

pub async fn a_scene_s_summary_is_what_is_kept(catalog: &impl SceneCatalog) {
    let id = a_scene(1_000);
    let forgotten_one = a_scene(2_000);
    remembered(catalog, id).await;
    retitled(catalog, id, "The loom, again", 2).await;
    retitled(catalog, id, "The loom", 1).await;
    opened(catalog, id, "Once there was", 5).await;
    opened(catalog, id, "Before that", 3).await;
    remembered(catalog, forgotten_one).await;
    forgotten(catalog, forgotten_one).await;

    let kept = catalog.summary_of(id).await.expect("asking should succeed");
    let gone = catalog
        .summary_of(forgotten_one)
        .await
        .expect("asking should succeed");
    let never = catalog
        .summary_of(a_scene(3_000))
        .await
        .expect("asking should succeed");

    assert_eq!(kept, Some(summary(id, "The loom, again", "Once there was")));
    assert_eq!((gone, never), (None, None));
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

async fn claimed(tasks: &impl QuotationRestylingTasks, project: &str, change: u64) -> bool {
    tasks
        .claim(&a_project(project), change)
        .await
        .expect("claiming should succeed")
}

pub async fn the_first_change_claims_the_book(tasks: &impl QuotationRestylingTasks) {
    assert!(claimed(tasks, "project_1", 3).await);
}

pub async fn a_newer_change_takes_the_book_from_an_older_one(tasks: &impl QuotationRestylingTasks) {
    claimed(tasks, "project_1", 3).await;

    assert!(claimed(tasks, "project_1", 5).await);
    assert!(
        !claimed(tasks, "project_1", 3).await,
        "the older change has lost the book for good"
    );
}

pub async fn an_older_change_is_turned_away_and_leaves_the_newer_one_holding(
    tasks: &impl QuotationRestylingTasks,
) {
    claimed(tasks, "project_1", 5).await;

    assert!(!claimed(tasks, "project_1", 3).await);
    assert!(claimed(tasks, "project_1", 5).await);
}

pub async fn the_same_change_claims_the_book_again(tasks: &impl QuotationRestylingTasks) {
    claimed(tasks, "project_1", 5).await;

    assert!(
        claimed(tasks, "project_1", 5).await,
        "every batch of one restyling claims the book anew"
    );
}

pub async fn another_book_is_claimed_on_its_own(tasks: &impl QuotationRestylingTasks) {
    claimed(tasks, "project_1", 5).await;

    assert!(claimed(tasks, "project_2", 3).await);
}

async fn begun(tasks: &impl QuotationRestylingTasks, change: u64, total: u64) -> bool {
    tasks
        .begin(&a_project("project_1"), change, total)
        .await
        .expect("beginning should succeed")
}

async fn progressed(tasks: &impl QuotationRestylingTasks, change: u64, done: u64, finished: bool) {
    tasks
        .progressed(&a_project("project_1"), change, done, finished)
        .await
        .expect("counting progress should succeed");
}

async fn progress(tasks: &impl QuotationRestylingTasks) -> Option<QuotationRestylingProgress> {
    tasks
        .progress(&a_project("project_1"))
        .await
        .expect("reading progress should succeed")
}

fn so_far(
    change: u64,
    total: u64,
    done: u64,
    finished: bool,
) -> Option<QuotationRestylingProgress> {
    Some(QuotationRestylingProgress {
        change,
        total,
        done,
        finished,
    })
}

pub async fn a_book_never_restyled_has_no_progress(tasks: &impl QuotationRestylingTasks) {
    assert_eq!(progress(tasks).await, None);
}

pub async fn a_restyling_counts_its_batches_until_it_is_finished(
    tasks: &impl QuotationRestylingTasks,
) {
    assert!(begun(tasks, 5, 7).await);

    progressed(tasks, 5, 4, false).await;
    assert_eq!(progress(tasks).await, so_far(5, 7, 4, false));

    progressed(tasks, 5, 3, true).await;
    assert_eq!(progress(tasks).await, so_far(5, 7, 7, true));
}

pub async fn a_newer_change_begins_counting_afresh(tasks: &impl QuotationRestylingTasks) {
    begun(tasks, 5, 7).await;
    progressed(tasks, 5, 7, true).await;

    assert!(begun(tasks, 6, 2).await);

    assert_eq!(progress(tasks).await, so_far(6, 2, 0, false));
}

pub async fn an_older_change_neither_begins_nor_counts(tasks: &impl QuotationRestylingTasks) {
    begun(tasks, 5, 7).await;

    assert!(!begun(tasks, 3, 9).await);
    progressed(tasks, 3, 2, true).await;

    assert_eq!(
        progress(tasks).await,
        so_far(5, 7, 0, false),
        "the bar shows the newest change, not what is left of an older one"
    );
}

pub async fn a_batch_claiming_the_book_keeps_the_count(tasks: &impl QuotationRestylingTasks) {
    begun(tasks, 5, 7).await;
    progressed(tasks, 5, 2, false).await;

    claimed(tasks, "project_1", 5).await;

    assert_eq!(progress(tasks).await, so_far(5, 7, 2, false));
}

pub async fn a_claim_nobody_began_counts_from_nothing(tasks: &impl QuotationRestylingTasks) {
    claimed(tasks, "project_1", 5).await;

    assert_eq!(progress(tasks).await, so_far(5, 0, 0, false));
}

pub async fn a_forgotten_tombstone_keeps_nothing_out_and_a_later_one_still_does(
    catalog: &impl SceneCatalog,
) {
    let long_gone = a_scene(1_000);
    let just_gone = a_scene(1_001);
    forgotten_at(catalog, long_gone, 1_000).await;
    forgotten_at(catalog, just_gone, 2_000).await;

    assert_eq!(forget_long_gone(catalog, 2_000, u64::MAX).await, 1);

    for scene in [long_gone, just_gone] {
        opened(catalog, scene, "It was late.", 3_000).await;
    }
    assert_eq!(
        listed(catalog, "project_1")
            .await
            .iter()
            .map(|summary| summary.id)
            .collect::<Vec<_>>(),
        vec![long_gone],
        "laid at the moment asked about is not before it"
    );
}

pub async fn a_scene_s_tombstone_is_laid_once_when_it_first_went(catalog: &impl SceneCatalog) {
    let scene = a_scene(1_000);
    forgotten_at(catalog, scene, 1_000).await;
    forgotten_at(catalog, scene, 5_000).await;

    assert_eq!(
        forget_long_gone(catalog, 2_000, u64::MAX).await,
        1,
        "a deletion heard again is the same deletion"
    );
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &impl SceneCatalog,
) {
    let newest = a_scene(1_000);
    let oldest = a_scene(1_001);
    let older = a_scene(1_002);
    for (scene, seconds) in [(newest, 1_002), (oldest, 1_000), (older, 1_001)] {
        forgotten_at(catalog, scene, seconds).await;
    }

    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 2);

    for scene in [newest, oldest, older] {
        opened(catalog, scene, "It was late.", 9_500).await;
    }
    let mut listed_now: Vec<SceneId> = listed(catalog, "project_1")
        .await
        .iter()
        .map(|summary| summary.id)
        .collect();
    listed_now.sort();
    let mut forgotten_first = vec![oldest, older];
    forgotten_first.sort();
    assert_eq!(listed_now, forgotten_first);
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 1);
}

macro_rules! restyling_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.tasks()).await;
            bench.cleanup().await;
        }
    };
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
        $crate::suite::conformance_case!($workbench, a_scene_s_summary_is_what_is_kept);
        $crate::suite::conformance_case!($workbench, a_forgotten_scene_is_not_listed);
        $crate::suite::conformance_case!(
            $workbench,
            nothing_arriving_after_a_scene_was_forgotten_brings_it_back
        );
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_what_was_never_remembered_is_harmless
        );
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
        $crate::suite::restyling_case!($workbench, the_first_change_claims_the_book);
        $crate::suite::restyling_case!($workbench, a_newer_change_takes_the_book_from_an_older_one);
        $crate::suite::restyling_case!(
            $workbench,
            an_older_change_is_turned_away_and_leaves_the_newer_one_holding
        );
        $crate::suite::restyling_case!($workbench, the_same_change_claims_the_book_again);
        $crate::suite::restyling_case!($workbench, another_book_is_claimed_on_its_own);
        $crate::suite::restyling_case!($workbench, a_book_never_restyled_has_no_progress);
        $crate::suite::restyling_case!(
            $workbench,
            a_restyling_counts_its_batches_until_it_is_finished
        );
        $crate::suite::restyling_case!($workbench, a_newer_change_begins_counting_afresh);
        $crate::suite::restyling_case!($workbench, an_older_change_neither_begins_nor_counts);
        $crate::suite::restyling_case!($workbench, a_batch_claiming_the_book_keeps_the_count);
        $crate::suite::restyling_case!($workbench, a_claim_nobody_began_counts_from_nothing);
    };
}

pub(crate) use {conformance_case, conformance_tests, restyling_case};
