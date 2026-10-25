use outline_core::{Attachment, OutlineCatalog, OutlineId, OutlineSummary, ProjectLink};
use time::OffsetDateTime;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: OutlineCatalog;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
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

pub async fn a_remembered_outline_is_listed_in_its_project(catalog: &impl OutlineCatalog) {
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

pub async fn a_project_that_never_opened_an_outline_lists_nothing(catalog: &impl OutlineCatalog) {
    let found = catalog
        .in_project(&ProjectLink::from("project_empty"))
        .await
        .expect("listing an unknown project should not fail");

    assert!(
        found.is_empty(),
        "an empty listing is what tells the service to start one"
    );
}

pub async fn outlines_of_other_projects_are_not_listed(catalog: &impl OutlineCatalog) {
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
    catalog: &impl OutlineCatalog,
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

pub async fn a_project_lists_its_outlines_in_a_settled_order(catalog: &impl OutlineCatalog) {
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

pub async fn a_scene_nobody_placed_is_in_no_outline(catalog: &impl OutlineCatalog) {
    let found = catalog
        .outlines_holding(&Attachment::scene("scene_loose"))
        .await
        .expect("looking should succeed");

    assert!(
        found.is_empty(),
        "a scene in the pool and not in the book is not an error, it is simply not in the book yet"
    );
}

pub async fn an_attached_scene_names_the_outline_holding_it(catalog: &impl OutlineCatalog) {
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

pub async fn everything_in_an_outline_names_it(catalog: &impl OutlineCatalog) {
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

pub async fn a_scene_may_sit_in_more_than_one_outline(catalog: &impl OutlineCatalog) {
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

pub async fn what_an_outline_holds_is_replaced_not_added_to(catalog: &impl OutlineCatalog) {
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
    catalog: &impl OutlineCatalog,
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
    catalog: &impl OutlineCatalog,
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

pub async fn a_forgotten_outline_takes_its_scenes_with_it(catalog: &impl OutlineCatalog) {
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
        .forget(&outline)
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

pub async fn forgetting_an_outline_nobody_opened_is_harmless(catalog: &impl OutlineCatalog) {
    catalog
        .forget(&OutlineId::generate(at(1_000)))
        .await
        .expect("forgetting an unknown outline should not fail");
}

async fn gone_but_held_by(catalog: &impl OutlineCatalog, outline: OutlineId) -> Vec<Attachment> {
    catalog
        .gone_but_held(outline)
        .await
        .expect("looking for what is gone should succeed")
}

async fn marked_gone(catalog: &impl OutlineCatalog, attachment: Attachment) {
    marked_gone_at(catalog, attachment, 1_000).await;
}

async fn marked_gone_at(catalog: &impl OutlineCatalog, attachment: Attachment, seconds: i64) {
    catalog
        .mark_gone(&attachment, at(seconds))
        .await
        .expect("marking gone should succeed");
}

pub async fn nothing_held_is_gone_until_marked(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    assert!(gone_but_held_by(catalog, outline).await.is_empty());
}

pub async fn a_held_scene_marked_gone_is_named(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(
            outline,
            &[Attachment::scene("scene_1"), Attachment::scene("scene_2")],
        )
        .await
        .expect("indexing should succeed");

    marked_gone(catalog, Attachment::scene("scene_1")).await;

    assert_eq!(
        gone_but_held_by(catalog, outline).await,
        vec![Attachment::scene("scene_1")]
    );
}

pub async fn a_scene_marked_gone_before_it_was_held_is_named(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));
    marked_gone(catalog, Attachment::scene("scene_1")).await;

    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        gone_but_held_by(catalog, outline).await,
        vec![Attachment::scene("scene_1")],
        "the deletion may be heard before the attachment is indexed"
    );
}

pub async fn a_gone_scene_let_go_is_no_longer_named(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");
    marked_gone(catalog, Attachment::scene("scene_1")).await;

    catalog
        .holds(outline, &[])
        .await
        .expect("indexing should succeed");

    assert!(gone_but_held_by(catalog, outline).await.is_empty());
}

pub async fn marking_the_same_scene_gone_twice_is_harmless(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    marked_gone(catalog, Attachment::scene("scene_1")).await;
    marked_gone(catalog, Attachment::scene("scene_1")).await;

    assert_eq!(
        gone_but_held_by(catalog, outline).await,
        vec![Attachment::scene("scene_1")]
    );
}

pub async fn a_gone_idea_is_not_mistaken_for_a_scene_of_the_same_id(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[Attachment::scene("shared_1")])
        .await
        .expect("indexing should succeed");

    marked_gone(catalog, Attachment::idea("shared_1")).await;

    assert!(gone_but_held_by(catalog, outline).await.is_empty());
}

async fn forget_long_gone(catalog: &impl OutlineCatalog, before_seconds: i64, at_most: u64) -> u64 {
    catalog
        .forget_tombstones_laid_before(at(before_seconds), at_most)
        .await
        .expect("forgetting tombstones should succeed")
}

pub async fn a_forgotten_tombstone_names_nothing_and_a_later_one_still_does(
    catalog: &impl OutlineCatalog,
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
    marked_gone_at(catalog, Attachment::scene("scene_long_gone"), 1_000).await;
    marked_gone_at(catalog, Attachment::idea("idea_just_gone"), 2_000).await;

    assert_eq!(forget_long_gone(catalog, 2_000, u64::MAX).await, 1);

    assert_eq!(
        gone_but_held_by(catalog, outline).await,
        vec![Attachment::idea("idea_just_gone")],
        "laid at the moment asked about is not before it"
    );
}

pub async fn a_tombstone_is_laid_once_when_what_it_marks_first_went(catalog: &impl OutlineCatalog) {
    marked_gone_at(catalog, Attachment::scene("scene_1"), 1_000).await;
    marked_gone_at(catalog, Attachment::scene("scene_1"), 5_000).await;

    assert_eq!(
        forget_long_gone(catalog, 2_000, u64::MAX).await,
        1,
        "a deletion heard again is the same deletion"
    );
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &impl OutlineCatalog,
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
        marked_gone_at(catalog, gone, seconds).await;
    }

    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 2);

    assert_eq!(gone_but_held_by(catalog, outline).await, vec![newest]);
    assert_eq!(forget_long_gone(catalog, 9_000, 2).await, 1);
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
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
    };
}
