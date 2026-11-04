use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};

use outline_core::{
    Attachment, CatalogError, OutlineCatalog, OutlineId, OutlineSummary, ProjectLink,
};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions, Tried};

static INDEXED: AtomicU64 = AtomicU64::new(1);

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: OutlineCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        Committing {
            catalog: self.store(),
            transactions: self.transactions(),
        }
    }

    async fn cleanup(self);
}

pub struct Committing<'a, C> {
    catalog: &'a C,
    transactions: &'a dyn Transactions,
}

fn unreachable(failure: transacting::Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

impl<C: OutlineCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn remember(&self, summary: &OutlineSummary) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .remember(transaction, summary)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget(
        &self,
        outline: &OutlineId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, outline, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn holds(
        &self,
        outline: OutlineId,
        attachments: &[Attachment],
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .holds(
                    transaction,
                    outline,
                    INDEXED.fetch_add(1, Ordering::Relaxed),
                    attachments,
                )
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn lay_tombstone(
        &self,
        attachment: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .lay_tombstone(transaction, attachment, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let done = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(done)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .lay_project_tombstone(transaction, project, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }
}

impl<C> Deref for Committing<'_, C> {
    type Target = C;

    fn deref(&self) -> &C {
        self.catalog
    }
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a plausible moment")
}

pub fn a_summary(id: OutlineId, project: &str) -> OutlineSummary {
    OutlineSummary {
        id,
        project: ProjectLink::from(project),
    }
}

pub async fn a_remembered_outline_is_listed_in_its_project(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let summary = a_summary(OutlineId::generate(at(1_000)), "project_1");

    catalog
        .remember(&summary)
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![summary]);
}

pub async fn a_project_that_never_opened_an_outline_lists_nothing(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let found = catalog
        .in_project(&ProjectLink::from("project_empty"))
        .await
        .expect("listing an unknown project should not fail");

    assert!(
        found.is_empty(),
        "an empty listing is what tells the service to start one"
    );
}

pub async fn outlines_of_other_projects_are_not_listed(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    catalog
        .remember(&a_summary(OutlineId::generate(at(1_000)), "project_mine"))
        .await
        .expect("remembering should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_theirs"))
        .await
        .expect("listing should succeed");

    assert!(found.is_empty());
}

pub async fn remembering_the_same_outline_again_replaces_what_was_there(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let id = OutlineId::generate(at(1_000));
    catalog
        .remember(&a_summary(id, "project_1"))
        .await
        .expect("remembering should succeed");

    catalog
        .remember(&a_summary(id, "project_1"))
        .await
        .expect("remembering again should succeed");

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(
        found.len(),
        1,
        "a projection is keyed by the outline, so a second write replaces rather than accrues"
    );
}

pub async fn a_project_lists_its_outlines_in_a_settled_order(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let opened: Vec<OutlineId> = (1..=6)
        .map(|nth| OutlineId::generate(at(nth * 1_000)))
        .collect();

    for id in opened.iter().rev() {
        catalog
            .remember(&a_summary(*id, "project_1"))
            .await
            .expect("remembering should succeed");
    }

    let found = catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed");

    assert_eq!(
        found
            .into_iter()
            .map(|outline| outline.id)
            .collect::<Vec<_>>(),
        opened,
        "one outline ships, but find-or-start must not pick a different one each time"
    );
}

async fn indexed_as_of(
    catalog: &Committing<'_, impl OutlineCatalog>,
    outline: OutlineId,
    version: u64,
    attachments: &[Attachment],
) {
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .holds(&mut transaction, outline, version, attachments)
        .await
        .expect("indexing should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

pub async fn an_older_index_heard_after_a_newer_one_changes_nothing(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    indexed_as_of(catalog, outline, 5, &[Attachment::scene("scene_5")]).await;

    indexed_as_of(catalog, outline, 4, &[Attachment::scene("scene_4")]).await;
    indexed_as_of(catalog, outline, 5, &[]).await;

    assert_eq!(
        catalog
            .outlines_holding(&Attachment::scene("scene_5"))
            .await
            .expect("looking should succeed"),
        vec![outline],
        "what an older read held is not what the outline holds now"
    );
    assert!(
        catalog
            .outlines_holding(&Attachment::scene("scene_4"))
            .await
            .expect("looking should succeed")
            .is_empty()
    );
    indexed_as_of(catalog, outline, 6, &[Attachment::scene("scene_6")]).await;
    assert!(
        catalog
            .outlines_holding(&Attachment::scene("scene_5"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "a newer one replaces it"
    );
}

pub async fn a_scene_nobody_placed_is_in_no_outline(catalog: &Committing<'_, impl OutlineCatalog>) {
    let found = catalog
        .outlines_holding(&Attachment::scene("scene_loose"))
        .await
        .expect("looking should succeed");

    assert!(
        found.is_empty(),
        "a scene in the pool and not in the book is not an error, it is simply not in the book yet"
    );
}

pub async fn an_attached_scene_names_the_outline_holding_it(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));

    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .outlines_holding(&Attachment::scene("scene_1"))
            .await
            .expect("looking should succeed"),
        vec![outline]
    );
}

pub async fn everything_in_an_outline_names_it(catalog: &Committing<'_, impl OutlineCatalog>) {
    let outline = OutlineId::generate(at(1_000));
    let held = [
        Attachment::scene("scene_1"),
        Attachment::scene("scene_2"),
        Attachment::idea("idea_1"),
    ];

    catalog
        .holds(outline, &held)
        .await
        .expect("indexing should succeed");

    for attachment in &held {
        assert_eq!(
            catalog
                .outlines_holding(attachment)
                .await
                .expect("looking should succeed"),
            vec![outline],
            "an outline holds everything it was given, not only the first"
        );
    }
}

pub async fn a_scene_may_sit_in_more_than_one_outline(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let earliest = OutlineId::generate(at(1_000));
    let latest = OutlineId::generate(at(2_000));
    for outline in [latest, earliest] {
        catalog
            .holds(outline, &[Attachment::scene("scene_1")])
            .await
            .expect("indexing should succeed");
    }

    assert_eq!(
        catalog
            .outlines_holding(&Attachment::scene("scene_1"))
            .await
            .expect("looking should succeed"),
        vec![earliest, latest],
        "one scene sits in at most one section, but the model allows a project several outlines"
    );
}

pub async fn what_an_outline_holds_is_replaced_not_added_to(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    catalog
        .holds(outline, &[Attachment::scene("scene_2")])
        .await
        .expect("indexing again should succeed");

    assert!(
        catalog
            .outlines_holding(&Attachment::scene("scene_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "the projector writes the whole set, so a detached scene falls out of the index"
    );
    assert_eq!(
        catalog
            .outlines_holding(&Attachment::scene("scene_2"))
            .await
            .expect("looking should succeed"),
        vec![outline]
    );
}

pub async fn what_stays_is_kept_what_left_is_dropped_and_what_came_is_added(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(
            outline,
            &[
                Attachment::scene("scene_1"),
                Attachment::scene("scene_2"),
                Attachment::idea("idea_1"),
            ],
        )
        .await
        .expect("indexing should succeed");

    catalog
        .holds(
            outline,
            &[
                Attachment::scene("scene_2"),
                Attachment::idea("idea_1"),
                Attachment::scene("scene_3"),
            ],
        )
        .await
        .expect("indexing again should succeed");

    let holding = |attachment: Attachment| async move {
        catalog
            .outlines_holding(&attachment)
            .await
            .expect("looking should succeed")
    };
    assert!(holding(Attachment::scene("scene_1")).await.is_empty());
    assert_eq!(
        holding(Attachment::scene("scene_2")).await,
        vec![outline],
        "only what changed is written, so what stays must not be lost on the way"
    );
    assert_eq!(holding(Attachment::idea("idea_1")).await, vec![outline]);
    assert_eq!(holding(Attachment::scene("scene_3")).await, vec![outline]);
}

pub async fn one_outline_letting_a_scene_go_leaves_the_others_holding_it(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let keeping = OutlineId::generate(at(1_000));
    let dropping = OutlineId::generate(at(2_000));
    for outline in [keeping, dropping] {
        catalog
            .holds(outline, &[Attachment::scene("scene_1")])
            .await
            .expect("indexing should succeed");
    }

    catalog
        .holds(dropping, &[])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .outlines_holding(&Attachment::scene("scene_1"))
            .await
            .expect("looking should succeed"),
        vec![keeping],
        "an index kept in both directions must not forget the outlines that still hold it"
    );
}

pub async fn a_forgotten_outline_takes_its_scenes_with_it(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    let elsewhere = OutlineId::generate(at(2_000));
    catalog
        .remember(&a_summary(outline, "project_1"))
        .await
        .expect("remembering should succeed");
    catalog
        .remember(&a_summary(elsewhere, "project_2"))
        .await
        .expect("remembering should succeed");
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");
    catalog
        .holds(elsewhere, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    catalog
        .forget(&outline, at(5_000))
        .await
        .expect("forgetting should succeed");

    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
    assert_eq!(
        catalog
            .outlines_holding(&Attachment::scene("scene_1"))
            .await
            .expect("looking should succeed"),
        vec![elsewhere],
        "a forgotten outline must take its scenes with it, or the index keeps answering for an \
         outline that is gone — and leave every other outline's alone"
    );
}

pub async fn forgetting_an_outline_nobody_opened_is_harmless(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    catalog
        .forget(&OutlineId::generate(at(1_000)), at(5_000))
        .await
        .expect("forgetting an unknown outline should not fail");
}

async fn held_though_tombstoned_by(
    catalog: &Committing<'_, impl OutlineCatalog>,
    outline: OutlineId,
) -> Vec<Attachment> {
    catalog
        .held_though_tombstoned(outline)
        .await
        .expect("looking for what is gone should succeed")
}

async fn tombstoned(catalog: &Committing<'_, impl OutlineCatalog>, attachment: Attachment) {
    tombstoned_at(catalog, attachment, 1_000).await;
}

async fn tombstoned_at(
    catalog: &Committing<'_, impl OutlineCatalog>,
    attachment: Attachment,
    seconds: i64,
) {
    catalog
        .lay_tombstone(&attachment, at(seconds))
        .await
        .expect("laying a tombstone should succeed");
}

pub async fn nothing_held_is_gone_until_marked(catalog: &Committing<'_, impl OutlineCatalog>) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    assert!(held_though_tombstoned_by(catalog, outline).await.is_empty());
}

pub async fn a_held_scene_marked_gone_is_named(catalog: &Committing<'_, impl OutlineCatalog>) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(
            outline,
            &[Attachment::scene("scene_1"), Attachment::scene("scene_2")],
        )
        .await
        .expect("indexing should succeed");

    tombstoned(catalog, Attachment::scene("scene_1")).await;

    assert_eq!(
        held_though_tombstoned_by(catalog, outline).await,
        vec![Attachment::scene("scene_1")]
    );
}

pub async fn a_scene_marked_gone_before_it_was_held_is_named(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    tombstoned(catalog, Attachment::scene("scene_1")).await;

    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        held_though_tombstoned_by(catalog, outline).await,
        vec![Attachment::scene("scene_1")],
        "the deletion may be heard before the attachment is indexed"
    );
}

pub async fn a_gone_scene_let_go_is_no_longer_named(catalog: &Committing<'_, impl OutlineCatalog>) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");
    tombstoned(catalog, Attachment::scene("scene_1")).await;

    catalog
        .holds(outline, &[])
        .await
        .expect("indexing should succeed");

    assert!(held_though_tombstoned_by(catalog, outline).await.is_empty());
}

pub async fn marking_the_same_scene_gone_twice_is_harmless(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    tombstoned(catalog, Attachment::scene("scene_1")).await;
    tombstoned(catalog, Attachment::scene("scene_1")).await;

    assert_eq!(
        held_though_tombstoned_by(catalog, outline).await,
        vec![Attachment::scene("scene_1")]
    );
}

pub async fn a_gone_idea_is_not_mistaken_for_a_scene_of_the_same_id(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("shared_1")])
        .await
        .expect("indexing should succeed");

    tombstoned(catalog, Attachment::idea("shared_1")).await;

    assert!(held_though_tombstoned_by(catalog, outline).await.is_empty());
}

async fn forget_long_gone(
    catalog: &Committing<'_, impl OutlineCatalog>,
    before_seconds: i64,
    at_most: u64,
) -> u64 {
    catalog
        .forget_tombstones_laid_before(at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

pub async fn a_forgotten_tombstone_names_nothing_and_a_later_one_still_does(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(
            outline,
            &[
                Attachment::scene("scene_long_gone"),
                Attachment::idea("idea_just_gone"),
            ],
        )
        .await
        .expect("indexing should succeed");
    tombstoned_at(catalog, Attachment::scene("scene_long_gone"), 1_000).await;
    tombstoned_at(catalog, Attachment::idea("idea_just_gone"), 2_000).await;

    assert_eq!(forget_long_gone(catalog, 2_000, u64::MAX).await, 1);

    assert_eq!(
        held_though_tombstoned_by(catalog, outline).await,
        vec![Attachment::idea("idea_just_gone")],
        "laid at the moment asked about is not before it"
    );
}

pub async fn a_tombstone_is_laid_once_when_what_it_marks_first_went(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    tombstoned_at(catalog, Attachment::scene("scene_1"), 1_000).await;
    tombstoned_at(catalog, Attachment::scene("scene_1"), 5_000).await;

    assert_eq!(
        forget_long_gone(catalog, 2_000, u64::MAX).await,
        1,
        "a deletion heard again is the same deletion"
    );
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let outline = OutlineId::generate(at(1_000));
    let newest = Attachment::scene("scene_newest");
    catalog
        .holds(
            outline,
            &[
                newest.clone(),
                Attachment::scene("scene_oldest"),
                Attachment::idea("idea_older"),
            ],
        )
        .await
        .expect("indexing should succeed");
    for (gone, seconds) in [
        (newest.clone(), 1_002),
        (Attachment::scene("scene_oldest"), 1_000),
        (Attachment::idea("idea_older"), 1_001),
    ] {
        tombstoned_at(catalog, gone, seconds).await;
    }

    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 2);

    assert_eq!(
        held_though_tombstoned_by(catalog, outline).await,
        vec![newest]
    );
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 1);
}

async fn listed(catalog: &Committing<'_, impl OutlineCatalog>) -> Vec<OutlineSummary> {
    catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("listing should succeed")
}

pub async fn a_forgotten_outline_heard_of_late_is_not_remembered_again(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let gone = a_summary(OutlineId::generate(at(1_000)), "project_1");
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");
    catalog
        .forget(&gone.id, at(5_000))
        .await
        .expect("forgetting should succeed");

    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");

    assert!(
        listed(catalog).await.is_empty(),
        "what was forgotten stays forgotten, however late or new a summary of it comes"
    );
}

pub async fn outline_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let (older, newer, latest) = (
        a_summary(OutlineId::generate(at(1_000)), "project_1"),
        a_summary(OutlineId::generate(at(2_000)), "project_1"),
        a_summary(OutlineId::generate(at(3_000)), "project_1"),
    );
    for (summary, laid) in [(&newer, 6_000), (&older, 5_000), (&latest, 9_000)] {
        catalog
            .forget(&summary.id, at(laid))
            .await
            .expect("forgetting should succeed");
    }

    let forgotten = catalog
        .forget_tombstones_laid_before(at(8_000), 1)
        .await
        .expect("forgetting tombstones should succeed");
    for summary in [&older, &newer, &latest] {
        catalog
            .remember(summary)
            .await
            .expect("remembering should succeed");
    }

    assert_eq!(forgotten, 1);
    assert_eq!(
        listed(catalog).await,
        vec![older],
        "only the oldest tombstone is gone, so only it can be remembered again"
    );
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let kept = a_summary(OutlineId::generate(at(1_000)), "project_1");
    catalog
        .remember(&kept)
        .await
        .expect("remembering should succeed");
    catalog
        .holds(kept.id, &[Attachment::scene("scene_1")])
        .await
        .expect("holding should succeed");
    let mut transaction = catalog.begun().await;
    let uncommitted = catalog.uncommitted();
    uncommitted
        .remember(
            &mut transaction,
            &a_summary(OutlineId::generate(at(2_000)), "project_1"),
        )
        .await
        .expect("remembering should succeed");
    uncommitted
        .holds(
            &mut transaction,
            kept.id,
            INDEXED.fetch_add(1, Ordering::Relaxed),
            &[Attachment::scene("scene_2")],
        )
        .await
        .expect("holding should succeed");
    uncommitted
        .lay_tombstone(&mut transaction, &Attachment::scene("scene_1"), at(3_000))
        .await
        .expect("laying a tombstone should succeed");
    uncommitted
        .forget(&mut transaction, &kept.id, at(3_000))
        .await
        .expect("forgetting should succeed");

    drop(transaction);

    assert_eq!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed"),
        vec![kept.clone()]
    );
    assert_eq!(
        catalog
            .outlines_holding(&Attachment::scene("scene_1"))
            .await
            .expect("asking should succeed"),
        vec![kept.id]
    );
    assert!(
        catalog
            .held_though_tombstoned(kept.id)
            .await
            .expect("asking should succeed")
            .is_empty()
    );
}

pub async fn outline_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let gone = a_summary(OutlineId::generate(at(1_000)), "project_1");
    catalog
        .forget(&gone.id, at(5_000))
        .await
        .expect("forgetting should succeed");
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .forget_tombstones_laid_before(&mut transaction, at(8_000), 10)
        .await
        .expect("forgetting tombstones should succeed");

    drop(transaction);
    catalog
        .remember(&gone)
        .await
        .expect("remembering should succeed");

    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

pub async fn a_project_has_no_tombstone_until_its_deletion_is_laid(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let (deleted, kept) = (
        ProjectLink::from("project_1"),
        ProjectLink::from("project_2"),
    );
    let had_one_before = catalog
        .has_project_tombstone(&deleted)
        .await
        .expect("asking should succeed");

    catalog
        .lay_project_tombstone(&deleted, at(5_000))
        .await
        .expect("laying a project's tombstone should succeed");

    assert!(!had_one_before, "a project nobody deleted has no tombstone");
    assert!(
        catalog
            .has_project_tombstone(&deleted)
            .await
            .expect("asking should succeed")
    );
    assert!(
        !catalog
            .has_project_tombstone(&kept)
            .await
            .expect("asking should succeed"),
        "one project's deletion buries no other"
    );
}

pub async fn project_tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let (older, newer, latest) = (
        ProjectLink::from("project_1"),
        ProjectLink::from("project_2"),
        ProjectLink::from("project_3"),
    );
    for (project, laid) in [(&newer, 6_000), (&older, 5_000), (&latest, 9_000)] {
        catalog
            .lay_project_tombstone(project, at(laid))
            .await
            .expect("laying a project's tombstone should succeed");
    }

    let forgotten = catalog
        .forget_tombstones_laid_before(at(8_000), 1)
        .await
        .expect("forgetting tombstones should succeed");

    let mut standing = Vec::new();
    for project in [&older, &newer, &latest] {
        standing.push(
            catalog
                .has_project_tombstone(project)
                .await
                .expect("asking should succeed"),
        );
    }
    assert_eq!(forgotten, 1);
    assert_eq!(
        standing,
        vec![false, true, true],
        "only the oldest tombstone is forgotten, and only one of those due"
    );
}

pub async fn a_project_tombstone_laid_in_a_transaction_that_is_dropped_is_not_there(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    let project = ProjectLink::from("project_1");
    let mut transaction = catalog.begun().await;
    catalog
        .uncommitted()
        .lay_project_tombstone(&mut transaction, &project, at(5_000))
        .await
        .expect("laying a project's tombstone should succeed");
    drop(transaction);

    assert!(
        !catalog
            .has_project_tombstone(&project)
            .await
            .expect("asking should succeed"),
        "a deletion heard in a hearing that did not commit was not heard"
    );
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::catalog_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::catalog_conformance_case!(
            $workbench,
            an_older_index_heard_after_a_newer_one_changes_nothing
        );
        $crate::catalog_conformance_case!(
            $workbench,
            outline_tombstones_forgotten_in_a_transaction_that_is_dropped_still_stand
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_outline_heard_of_late_is_not_remembered_again
        );
        $crate::catalog_conformance_case!(
            $workbench,
            outline_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!($workbench, a_forgotten_outline_takes_its_scenes_with_it);
        $crate::catalog_conformance_case!(
            $workbench,
            forgetting_an_outline_nobody_opened_is_harmless
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_remembered_outline_is_listed_in_its_project
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_that_never_opened_an_outline_lists_nothing
        );
        $crate::catalog_conformance_case!($workbench, outlines_of_other_projects_are_not_listed);
        $crate::catalog_conformance_case!(
            $workbench,
            remembering_the_same_outline_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_lists_its_outlines_in_a_settled_order
        );
        $crate::catalog_conformance_case!($workbench, a_scene_nobody_placed_is_in_no_outline);
        $crate::catalog_conformance_case!(
            $workbench,
            an_attached_scene_names_the_outline_holding_it
        );
        $crate::catalog_conformance_case!($workbench, a_scene_may_sit_in_more_than_one_outline);
        $crate::catalog_conformance_case!(
            $workbench,
            what_an_outline_holds_is_replaced_not_added_to
        );
        $crate::catalog_conformance_case!(
            $workbench,
            what_stays_is_kept_what_left_is_dropped_and_what_came_is_added
        );
        $crate::catalog_conformance_case!(
            $workbench,
            one_outline_letting_a_scene_go_leaves_the_others_holding_it
        );
        $crate::catalog_conformance_case!($workbench, everything_in_an_outline_names_it);
        $crate::catalog_conformance_case!($workbench, nothing_held_is_gone_until_marked);
        $crate::catalog_conformance_case!($workbench, a_held_scene_marked_gone_is_named);
        $crate::catalog_conformance_case!(
            $workbench,
            a_scene_marked_gone_before_it_was_held_is_named
        );
        $crate::catalog_conformance_case!($workbench, a_gone_scene_let_go_is_no_longer_named);
        $crate::catalog_conformance_case!(
            $workbench,
            marking_the_same_scene_gone_twice_is_harmless
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_gone_idea_is_not_mistaken_for_a_scene_of_the_same_id
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_forgotten_tombstone_names_nothing_and_a_later_one_still_does
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_tombstone_is_laid_once_when_what_it_marks_first_went
        );
        $crate::catalog_conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_has_no_tombstone_until_its_deletion_is_laid
        );
        $crate::catalog_conformance_case!(
            $workbench,
            project_tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
        $crate::catalog_conformance_case!(
            $workbench,
            a_project_tombstone_laid_in_a_transaction_that_is_dropped_is_not_there
        );
    };
}
