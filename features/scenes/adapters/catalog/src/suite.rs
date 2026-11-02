use std::ops::Deref;

use scenes_core::{
    CatalogError, Page, ProjectLink, QuotationRestylingProgress, QuotationRestylingStanding,
    QuotationRestylingTaskError, QuotationRestylingTasks, SceneCatalog, SceneId, SceneSummary,
    SceneTitle,
};
use time::{Duration, OffsetDateTime};
use transacting::{Transaction, Transactions};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: SceneCatalog;
    type Tasks: QuotationRestylingTasks;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    fn tasks(&self) -> &Self::Tasks;

    fn transactions(&self) -> &dyn Transactions;

    fn committing_catalog(&self) -> Committing<'_, Self::Catalog> {
        Committing {
            kept: self.catalog(),
            transactions: self.transactions(),
        }
    }

    fn committing_tasks(&self) -> Committing<'_, Self::Tasks> {
        Committing {
            kept: self.tasks(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, K> {
    kept: &'a K,
    transactions: &'a dyn Transactions,
}

impl<K> Committing<'_, K> {
    async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    async fn finished(transaction: Transaction) {
        transaction
            .finish()
            .await
            .expect("finishing should succeed");
    }

    fn uncommitted(&self) -> &K {
        self.kept
    }
}

impl<K: SceneCatalog> Committing<'_, K> {
    async fn remember(&self, id: SceneId, project: &ProjectLink) -> Result<(), CatalogError> {
        let mut transaction = self.begun().await;
        self.kept.remember(&mut transaction, id, project).await?;
        Self::finished(transaction).await;

        Ok(())
    }

    async fn retitle(
        &self,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        let mut transaction = self.begun().await;
        self.kept
            .retitle(&mut transaction, id, project, title, version)
            .await?;
        Self::finished(transaction).await;

        Ok(())
    }

    async fn note_opening(
        &self,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut transaction = self.begun().await;
        self.kept
            .note_opening(&mut transaction, id, project, opening, at)
            .await?;
        Self::finished(transaction).await;

        Ok(())
    }

    async fn forget(&self, id: SceneId, at: OffsetDateTime) -> Result<(), CatalogError> {
        let mut transaction = self.begun().await;
        self.kept.forget(&mut transaction, id, at).await?;
        Self::finished(transaction).await;

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut transaction = self.begun().await;
        let forgotten = self
            .kept
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        Self::finished(transaction).await;

        Ok(forgotten)
    }
}

impl<K: QuotationRestylingTasks> Committing<'_, K> {
    async fn begin(
        &self,
        project: &ProjectLink,
        change: u64,
        total: u64,
        cursor: Option<&str>,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let mut transaction = self.begun().await;
        let begun = self
            .kept
            .begin(&mut transaction, project, change, total, cursor)
            .await?;
        Self::finished(transaction).await;

        Ok(begun)
    }

    async fn standing(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingStanding>, QuotationRestylingTaskError> {
        let mut transaction = self.begun().await;
        let standing = self.kept.standing(&mut transaction, project).await?;
        Self::finished(transaction).await;

        Ok(standing)
    }

    async fn progressed(
        &self,
        project: &ProjectLink,
        change: u64,
        page: Page<'_>,
        finished: bool,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let mut transaction = self.begun().await;
        let counted = self
            .kept
            .progressed(&mut transaction, project, change, page, finished)
            .await?;
        Self::finished(transaction).await;

        Ok(counted)
    }
}

impl<K> Deref for Committing<'_, K> {
    type Target = K;

    fn deref(&self) -> &K {
        self.kept
    }
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

async fn remembered(catalog: &Committing<'_, impl SceneCatalog>, id: SceneId) {
    catalog
        .remember(id, &a_project("project_1"))
        .await
        .expect("remembering should succeed");
}

async fn retitled(
    catalog: &Committing<'_, impl SceneCatalog>,
    id: SceneId,
    title: &str,
    version: u64,
) {
    catalog
        .retitle(id, &a_project("project_1"), &a_title(title), version)
        .await
        .expect("retitling should succeed");
}

async fn opened(
    catalog: &Committing<'_, impl SceneCatalog>,
    id: SceneId,
    opening: &str,
    seconds: i64,
) {
    catalog
        .note_opening(id, &a_project("project_1"), opening, at(seconds))
        .await
        .expect("noting an opening should succeed");
}

async fn forgotten(catalog: &Committing<'_, impl SceneCatalog>, id: SceneId) {
    forgotten_at(catalog, id, 10_000).await;
}

async fn forgotten_at(catalog: &Committing<'_, impl SceneCatalog>, id: SceneId, seconds: i64) {
    catalog
        .forget(id, at(seconds))
        .await
        .expect("forgetting should succeed");
}

async fn forget_long_gone(
    catalog: &Committing<'_, impl SceneCatalog>,
    before_seconds: i64,
    at_most: u64,
) -> u64 {
    catalog
        .forget_tombstones_laid_before(at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

async fn listed(catalog: &Committing<'_, impl SceneCatalog>, project: &str) -> Vec<SceneSummary> {
    catalog
        .in_project_after(&a_project(project), None, 100)
        .await
        .expect("listing should succeed")
}

pub async fn a_project_nobody_wrote_in_lists_nothing(catalog: &Committing<'_, impl SceneCatalog>) {
    assert!(listed(catalog, "project_1").await.is_empty());
}

pub async fn a_remembered_scene_is_listed_untitled_and_without_an_opening(
    catalog: &Committing<'_, impl SceneCatalog>,
) {
    let id = a_scene(1_000);

    remembered(catalog, id).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "", "")]
    );
}

pub async fn another_project_sees_none_of_them(catalog: &Committing<'_, impl SceneCatalog>) {
    remembered(catalog, a_scene(1_000)).await;

    assert!(listed(catalog, "project_2").await.is_empty());
}

pub async fn scenes_are_listed_in_order_a_page_at_a_time(
    catalog: &Committing<'_, impl SceneCatalog>,
) {
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

pub async fn a_retitle_names_the_scene(catalog: &Committing<'_, impl SceneCatalog>) {
    let id = a_scene(1_000);
    remembered(catalog, id).await;

    retitled(catalog, id, "The loom", 1).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "The loom", "")]
    );
}

pub async fn a_late_retitle_does_not_undo_a_newer_one(catalog: &Committing<'_, impl SceneCatalog>) {
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

pub async fn an_opening_is_kept_beside_the_title(catalog: &Committing<'_, impl SceneCatalog>) {
    let id = a_scene(1_000);
    remembered(catalog, id).await;
    retitled(catalog, id, "The loom", 1).await;

    opened(catalog, id, "The loom stood silent.", 1_000).await;

    assert_eq!(
        listed(catalog, "project_1").await,
        vec![summary(id, "The loom", "The loom stood silent.")]
    );
}

pub async fn a_late_opening_does_not_undo_a_newer_one(catalog: &Committing<'_, impl SceneCatalog>) {
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
    catalog: &Committing<'_, impl SceneCatalog>,
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

pub async fn a_scene_s_summary_is_what_is_kept(catalog: &Committing<'_, impl SceneCatalog>) {
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

pub async fn a_forgotten_scene_is_not_listed(catalog: &Committing<'_, impl SceneCatalog>) {
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
    catalog: &Committing<'_, impl SceneCatalog>,
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

pub async fn forgetting_what_was_never_remembered_is_harmless(
    catalog: &Committing<'_, impl SceneCatalog>,
) {
    let id = a_scene(1_000);

    forgotten(catalog, id).await;
    forgotten(catalog, id).await;

    assert!(listed(catalog, "project_1").await.is_empty());
}

async fn begun(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
    project: &str,
    change: u64,
    total: u64,
) -> bool {
    tasks
        .begin(&a_project(project), change, total, None)
        .await
        .expect("beginning should succeed")
}

async fn standing(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
    project: &str,
) -> Option<QuotationRestylingStanding> {
    tasks
        .standing(&a_project(project))
        .await
        .expect("asking how a restyling stands should succeed")
}

async fn progressed(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
    change: u64,
    from: Option<&str>,
    to: Option<&str>,
    done: u64,
    finished: bool,
) -> bool {
    tasks
        .progressed(
            &a_project("project_1"),
            change,
            Page { from, to, done },
            finished,
        )
        .await
        .expect("counting progress should succeed")
}

async fn progress(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) -> Option<QuotationRestylingProgress> {
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

fn standing_at(
    change: u64,
    cursor: Option<&str>,
    finished: bool,
) -> Option<QuotationRestylingStanding> {
    Some(QuotationRestylingStanding {
        change,
        cursor: cursor.map(str::to_owned),
        finished,
    })
}

pub async fn the_first_change_begins_the_book(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    assert!(begun(tasks, "project_1", 3, 7).await);
    assert_eq!(
        standing(tasks, "project_1").await,
        standing_at(3, None, false)
    );
}

pub async fn a_newer_change_takes_the_book_from_an_older_one(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    begun(tasks, "project_1", 3, 7).await;

    assert!(begun(tasks, "project_1", 5, 7).await);
    assert!(
        !begun(tasks, "project_1", 3, 7).await,
        "the older change has lost the book for good"
    );
    assert_eq!(
        standing(tasks, "project_1").await,
        standing_at(5, None, false)
    );
}

pub async fn the_same_change_begun_again_keeps_how_far_it_got(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    begun(tasks, "project_1", 5, 7).await;
    progressed(tasks, 5, None, Some("scene_3"), 3, false).await;

    assert!(
        !begun(tasks, "project_1", 5, 7).await,
        "a first message heard again must not start the count over"
    );
    assert_eq!(progress(tasks).await, so_far(5, 7, 3, false));
    assert_eq!(
        standing(tasks, "project_1").await,
        standing_at(5, Some("scene_3"), false)
    );
}

pub async fn another_book_is_begun_on_its_own(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    begun(tasks, "project_1", 5, 7).await;

    assert!(begun(tasks, "project_2", 3, 1).await);
}

pub async fn a_book_never_restyled_has_no_progress(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    assert_eq!(progress(tasks).await, None);
    assert_eq!(standing(tasks, "project_1").await, None);
}

pub async fn a_restyling_counts_its_pages_until_it_is_finished(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    assert!(begun(tasks, "project_1", 5, 7).await);

    assert!(progressed(tasks, 5, None, Some("scene_4"), 4, false).await);
    assert_eq!(progress(tasks).await, so_far(5, 7, 4, false));

    assert!(progressed(tasks, 5, Some("scene_4"), Some("scene_7"), 3, true).await);
    assert_eq!(progress(tasks).await, so_far(5, 7, 7, true));
    assert_eq!(
        standing(tasks, "project_1").await,
        standing_at(5, Some("scene_7"), true)
    );
}

pub async fn a_page_counted_twice_is_counted_once(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    begun(tasks, "project_1", 5, 7).await;
    progressed(tasks, 5, None, Some("scene_4"), 4, false).await;

    assert!(
        !progressed(tasks, 5, None, Some("scene_4"), 4, false).await,
        "the page was counted when the cursor stood where it began"
    );
    assert_eq!(progress(tasks).await, so_far(5, 7, 4, false));
}

pub async fn a_finished_restyling_counts_no_more(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    begun(tasks, "project_1", 5, 1).await;
    progressed(tasks, 5, None, None, 0, true).await;

    assert!(!progressed(tasks, 5, None, None, 0, true).await);
    assert_eq!(progress(tasks).await, so_far(5, 1, 0, true));
}

pub async fn a_newer_change_begins_counting_afresh(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    begun(tasks, "project_1", 5, 7).await;
    progressed(tasks, 5, None, Some("scene_7"), 7, true).await;

    assert!(begun(tasks, "project_1", 6, 2).await);

    assert_eq!(progress(tasks).await, so_far(6, 2, 0, false));
    assert_eq!(
        standing(tasks, "project_1").await,
        standing_at(6, None, false)
    );
}

pub async fn an_older_change_neither_begins_nor_counts(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    begun(tasks, "project_1", 5, 7).await;

    assert!(!begun(tasks, "project_1", 3, 9).await);
    assert!(!progressed(tasks, 3, None, Some("scene_2"), 2, true).await);

    assert_eq!(
        progress(tasks).await,
        so_far(5, 7, 0, false),
        "the bar shows the newest change, not what is left of an older one"
    );
}

pub async fn a_continuation_nobody_began_counts_from_its_cursor(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    assert!(
        tasks
            .begin(&a_project("project_1"), 5, 0, Some("scene_3"))
            .await
            .expect("beginning should succeed")
    );

    assert_eq!(progress(tasks).await, so_far(5, 0, 0, false));
    assert_eq!(
        standing(tasks, "project_1").await,
        standing_at(5, Some("scene_3"), false)
    );
}

pub async fn a_forgotten_tombstone_keeps_nothing_out_and_a_later_one_still_does(
    catalog: &Committing<'_, impl SceneCatalog>,
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

pub async fn a_scene_s_tombstone_is_laid_once_when_it_first_went(
    catalog: &Committing<'_, impl SceneCatalog>,
) {
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
    catalog: &Committing<'_, impl SceneCatalog>,
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

pub async fn what_was_catalogued_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl SceneCatalog>,
) {
    let kept = a_scene(1_000);
    catalog
        .remember(kept, &a_project("project_1"))
        .await
        .expect("remembering should succeed");
    let mut transaction = catalog.begun().await;
    let uncommitted = catalog.uncommitted();
    uncommitted
        .remember(&mut transaction, a_scene(2_000), &a_project("project_1"))
        .await
        .expect("remembering should succeed");
    uncommitted
        .retitle(
            &mut transaction,
            kept,
            &a_project("project_1"),
            &SceneTitle::new("Never titled").expect("a plain title"),
            1,
        )
        .await
        .expect("retitling should succeed");
    uncommitted
        .forget(&mut transaction, kept, at(3_000))
        .await
        .expect("forgetting should succeed");

    drop(transaction);

    let listed = catalog
        .in_project_after(&a_project("project_1"), None, 10)
        .await
        .expect("listing should succeed");
    assert_eq!(
        listed.iter().map(|summary| summary.id).collect::<Vec<_>>(),
        vec![kept]
    );
    assert_eq!(listed[0].title, SceneTitle::untitled());
}

pub async fn a_claim_made_in_a_transaction_that_is_dropped_was_never_made(
    tasks: &Committing<'_, impl QuotationRestylingTasks>,
) {
    let mut transaction = tasks.begun().await;
    tasks
        .uncommitted()
        .begin(&mut transaction, &a_project("project_1"), 5, 10, None)
        .await
        .expect("beginning should succeed");

    drop(transaction);

    assert_eq!(
        tasks
            .progress(&a_project("project_1"))
            .await
            .expect("asking should succeed"),
        None
    );
}

macro_rules! restyling_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing_tasks()).await;
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
            $crate::suite::$case(&bench.committing_catalog()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::suite::conformance_case!(
            $workbench,
            what_was_catalogued_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::suite::restyling_case!(
            $workbench,
            a_claim_made_in_a_transaction_that_is_dropped_was_never_made
        );
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
        $crate::suite::restyling_case!($workbench, the_first_change_begins_the_book);
        $crate::suite::restyling_case!($workbench, a_newer_change_takes_the_book_from_an_older_one);
        $crate::suite::restyling_case!(
            $workbench,
            the_same_change_begun_again_keeps_how_far_it_got
        );
        $crate::suite::restyling_case!($workbench, another_book_is_begun_on_its_own);
        $crate::suite::restyling_case!($workbench, a_book_never_restyled_has_no_progress);
        $crate::suite::restyling_case!(
            $workbench,
            a_restyling_counts_its_pages_until_it_is_finished
        );
        $crate::suite::restyling_case!($workbench, a_page_counted_twice_is_counted_once);
        $crate::suite::restyling_case!($workbench, a_finished_restyling_counts_no_more);
        $crate::suite::restyling_case!($workbench, a_newer_change_begins_counting_afresh);
        $crate::suite::restyling_case!($workbench, an_older_change_neither_begins_nor_counts);
        $crate::suite::restyling_case!(
            $workbench,
            a_continuation_nobody_began_counts_from_its_cursor
        );
    };
}

pub(crate) use {conformance_case, conformance_tests, restyling_case};
