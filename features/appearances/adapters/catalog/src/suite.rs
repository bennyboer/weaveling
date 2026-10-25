use appearances_core::{
    AppearanceCatalog, IdeaLink, Place, ProjectLink, SceneLink, SectionLink, Subject,
};
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

const LAID: OffsetDateTime = datetime!(2026-10-25 12:00 UTC);

const ALL_AT_ONCE: u64 = u64::MAX;

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: AppearanceCatalog;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    async fn cleanup(self);
}

fn an_idea(named: &str) -> Subject {
    Subject::Idea(IdeaLink::from(named))
}

fn a_section(named: &str) -> Place {
    Place::Section(SectionLink::from(named))
}

fn a_scene(named: &str) -> Place {
    Place::Scene(SceneLink::from(named))
}

async fn remembered(catalog: &impl AppearanceCatalog, idea: &str, places: &[Place]) {
    for place in places {
        catalog
            .remember(&an_idea(idea), place, 1)
            .await
            .expect("remembering should succeed");
    }
}

async fn places_of(catalog: &impl AppearanceCatalog, idea: &str) -> Vec<Place> {
    catalog
        .places_of(&an_idea(idea))
        .await
        .expect("asking should succeed")
}

pub async fn an_idea_nobody_placed_appears_nowhere(catalog: &impl AppearanceCatalog) {
    assert!(places_of(catalog, "idea_1").await.is_empty());
}

pub async fn an_idea_appears_wherever_it_was_remembered(catalog: &impl AppearanceCatalog) {
    remembered(
        catalog,
        "idea_1",
        &[a_scene("scene_1"), a_section("section_1")],
    )
    .await;

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_scene("scene_1"), a_section("section_1")]
    );
}

pub async fn places_come_back_in_one_order_whatever_order_they_were_remembered_in(
    catalog: &impl AppearanceCatalog,
) {
    remembered(
        catalog,
        "idea_1",
        &[
            a_section("section_2"),
            a_scene("scene_1"),
            a_section("section_1"),
        ],
    )
    .await;

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![
            a_scene("scene_1"),
            a_section("section_1"),
            a_section("section_2"),
        ],
        "remembered shuffled on purpose: scenes come back before sections, each by id, \
         so both backends answer in the same order"
    );
}

pub async fn remembering_the_same_place_twice_is_one_appearance(catalog: &impl AppearanceCatalog) {
    remembered(
        catalog,
        "idea_1",
        &[a_section("section_1"), a_section("section_1")],
    )
    .await;

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_1")],
        "a redelivered attachment must not make an idea appear twice"
    );
}

pub async fn forgetting_one_place_leaves_the_others(catalog: &impl AppearanceCatalog) {
    remembered(
        catalog,
        "idea_1",
        &[a_section("section_1"), a_scene("scene_1")],
    )
    .await;

    catalog
        .forget(&an_idea("idea_1"), &a_section("section_1"), 2)
        .await
        .expect("forgetting should succeed");

    assert_eq!(places_of(catalog, "idea_1").await, vec![a_scene("scene_1")]);
}

pub async fn forgetting_an_idea_leaves_every_other_idea(catalog: &impl AppearanceCatalog) {
    remembered(catalog, "idea_gone", &[a_section("section_1")]).await;
    remembered(catalog, "idea_kept", &[a_section("section_1")]).await;

    catalog
        .forget_subject(&an_idea("idea_gone"), LAID)
        .await
        .expect("forgetting should succeed");

    assert!(places_of(catalog, "idea_gone").await.is_empty());
    assert_eq!(
        places_of(catalog, "idea_kept").await,
        vec![a_section("section_1")],
        "one idea being discarded must never empty the section for the others"
    );
}

pub async fn forgetting_a_place_reaches_every_idea_in_it(catalog: &impl AppearanceCatalog) {
    remembered(
        catalog,
        "idea_1",
        &[a_section("section_gone"), a_section("section_kept")],
    )
    .await;
    remembered(catalog, "idea_2", &[a_section("section_gone")]).await;

    catalog
        .forget_place(&a_section("section_gone"), LAID)
        .await
        .expect("forgetting should succeed");

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_kept")]
    );
    assert!(
        places_of(catalog, "idea_2").await.is_empty(),
        "a removed section takes every idea noted in it, not just the first one found"
    );
}

pub async fn forgetting_a_place_says_which_ideas_appeared_there(catalog: &impl AppearanceCatalog) {
    remembered(catalog, "idea_2", &[a_section("section_gone")]).await;
    remembered(catalog, "idea_1", &[a_section("section_gone")]).await;
    remembered(catalog, "idea_3", &[a_section("section_gone")]).await;
    catalog
        .forget(&an_idea("idea_3"), &a_section("section_gone"), 2)
        .await
        .expect("forgetting should succeed");
    remembered(catalog, "idea_4", &[a_section("section_kept")]).await;

    let appeared = catalog
        .forget_place(&a_section("section_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    let nobody = catalog
        .forget_place(&a_section("section_empty"), LAID)
        .await
        .expect("forgetting should succeed");

    assert_eq!(
        appeared,
        vec![an_idea("idea_1"), an_idea("idea_2")],
        "an idea already taken out of the place did not appear there"
    );
    assert!(nobody.is_empty());
}

pub async fn a_section_and_a_scene_sharing_an_id_are_different_places(
    catalog: &impl AppearanceCatalog,
) {
    remembered(catalog, "idea_1", &[a_section("same_1"), a_scene("same_1")]).await;

    catalog
        .forget_place(&a_scene("same_1"), LAID)
        .await
        .expect("forgetting should succeed");

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("same_1")],
        "the type is part of a place's identity"
    );
}

pub async fn forgetting_what_was_never_remembered_is_harmless(catalog: &impl AppearanceCatalog) {
    catalog
        .forget(&an_idea("idea_1"), &a_section("section_1"), 1)
        .await
        .expect("forgetting nothing is not a failure");
    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting nothing is not a failure");
    catalog
        .forget_place(&a_section("section_1"), LAID)
        .await
        .expect("forgetting nothing is not a failure");
}

async fn seen(catalog: &impl AppearanceCatalog, at: &Place, changes: &[(bool, u64)]) {
    for (present, version) in changes {
        if *present {
            catalog
                .remember(&an_idea("idea_1"), at, *version)
                .await
                .expect("remembering should succeed");
        } else {
            catalog
                .forget(&an_idea("idea_1"), at, *version)
                .await
                .expect("forgetting should succeed");
        }
    }
}

pub async fn a_late_remembering_does_not_undo_a_later_forgetting(catalog: &impl AppearanceCatalog) {
    seen(
        catalog,
        &a_section("section_1"),
        &[(true, 1), (false, 3), (true, 2)],
    )
    .await;

    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "an attachment tried again after its detachment must not put the idea back"
    );
}

pub async fn a_late_forgetting_does_not_undo_a_later_remembering(catalog: &impl AppearanceCatalog) {
    seen(
        catalog,
        &a_section("section_1"),
        &[(true, 1), (false, 2), (true, 3), (false, 2)],
    )
    .await;

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_1")],
        "a detachment tried again after the idea was attached anew must not take it away"
    );
}

pub async fn nothing_comes_back_to_a_forgotten_place(catalog: &impl AppearanceCatalog) {
    seen(catalog, &a_section("section_1"), &[(true, 1)]).await;
    catalog
        .forget_place(&a_section("section_1"), LAID)
        .await
        .expect("forgetting should succeed");

    seen(catalog, &a_section("section_1"), &[(true, 9)]).await;

    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "a removed section is gone for good, whatever arrives about it later"
    );
}

pub async fn a_forgotten_idea_comes_back_nowhere(catalog: &impl AppearanceCatalog) {
    seen(catalog, &a_section("section_1"), &[(true, 1)]).await;
    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");

    seen(catalog, &a_section("section_2"), &[(true, 9)]).await;

    assert!(
        places_of(catalog, "idea_1").await.is_empty(),
        "a discarded idea is gone for good, and its versions come from other streams, so only \
         remembering that it went can keep it out"
    );
}

pub async fn an_idea_s_project_is_kept_and_none_known_is_none(catalog: &impl AppearanceCatalog) {
    catalog
        .remember_project(&an_idea("idea_1"), &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");
    catalog
        .remember_project(&an_idea("idea_2"), &ProjectLink::from("project_2"))
        .await
        .expect("remembering should succeed");

    for (idea, project) in [
        ("idea_1", Some(ProjectLink::from("project_1"))),
        ("idea_2", Some(ProjectLink::from("project_2"))),
        ("idea_never", None),
    ] {
        assert_eq!(
            catalog
                .project_of(&an_idea(idea))
                .await
                .expect("asking should succeed"),
            project,
            "{idea}"
        );
    }
}

pub async fn a_discarded_idea_s_project_outlives_it_as_its_tombstone_does(
    catalog: &impl AppearanceCatalog,
) {
    catalog
        .remember_project(&an_idea("idea_1"), &ProjectLink::from("project_1"))
        .await
        .expect("remembering should succeed");

    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");

    assert_eq!(
        catalog
            .project_of(&an_idea("idea_1"))
            .await
            .expect("asking should succeed"),
        Some(ProjectLink::from("project_1")),
        "whoever followed the idea is told it appears nowhere now, which only who may read its \
         project may hear"
    );
}

pub async fn remembering_an_idea_s_project_again_keeps_one_the_last(
    catalog: &impl AppearanceCatalog,
) {
    for project in ["project_1", "project_1", "project_2"] {
        catalog
            .remember_project(&an_idea("idea_1"), &ProjectLink::from(project))
            .await
            .expect("remembering again is no failure");
    }

    assert_eq!(
        catalog
            .project_of(&an_idea("idea_1"))
            .await
            .expect("asking should succeed"),
        Some(ProjectLink::from("project_2"))
    );
}

pub async fn tombstones_laid_before_a_moment_are_forgotten_and_the_later_kept(
    catalog: &impl AppearanceCatalog,
) {
    let before = LAID + Duration::DAY;
    catalog
        .forget_subject(&an_idea("idea_long_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_place(&a_section("section_long_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_subject(&an_idea("idea_just_gone"), before)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_place(&a_section("section_just_gone"), before)
        .await
        .expect("forgetting should succeed");

    let forgotten = catalog
        .forget_tombstones_laid_before(before, ALL_AT_ONCE)
        .await
        .expect("forgetting tombstones should succeed");

    assert_eq!(forgotten, 2);
    for (idea, place) in [
        ("idea_long_gone", a_scene("scene_1")),
        ("idea_just_gone", a_scene("scene_1")),
        ("idea_1", a_section("section_long_gone")),
        ("idea_1", a_section("section_just_gone")),
    ] {
        remembered(catalog, idea, &[place]).await;
    }
    assert_eq!(
        places_of(catalog, "idea_long_gone").await,
        vec![a_scene("scene_1")],
        "a tombstone forgotten keeps nothing out"
    );
    assert!(places_of(catalog, "idea_just_gone").await.is_empty());
    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_section("section_long_gone")]
    );
}

pub async fn a_tombstone_is_laid_once_when_what_it_marks_first_went(
    catalog: &impl AppearanceCatalog,
) {
    catalog
        .forget_subject(&an_idea("idea_1"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_place(&a_section("section_1"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_subject(&an_idea("idea_1"), LAID + Duration::DAY * 2)
        .await
        .expect("hearing it again is no failure");
    catalog
        .forget_place(&a_section("section_1"), LAID + Duration::DAY * 2)
        .await
        .expect("hearing it again is no failure");

    assert_eq!(
        catalog
            .forget_tombstones_laid_before(LAID + Duration::DAY, ALL_AT_ONCE)
            .await
            .expect("forgetting tombstones should succeed"),
        2,
        "a discard heard again is the same discard"
    );
}

pub async fn a_long_gone_idea_s_project_is_forgotten_with_its_tombstone(
    catalog: &impl AppearanceCatalog,
) {
    for idea in ["idea_long_gone", "idea_just_gone", "idea_here"] {
        catalog
            .remember_project(&an_idea(idea), &ProjectLink::from("project_1"))
            .await
            .expect("remembering should succeed");
    }
    catalog
        .forget_subject(&an_idea("idea_long_gone"), LAID)
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_subject(&an_idea("idea_just_gone"), LAID + Duration::DAY)
        .await
        .expect("forgetting should succeed");

    catalog
        .forget_tombstones_laid_before(LAID + Duration::DAY, ALL_AT_ONCE)
        .await
        .expect("forgetting tombstones should succeed");

    for (idea, project) in [
        ("idea_long_gone", None),
        ("idea_just_gone", Some(ProjectLink::from("project_1"))),
        ("idea_here", Some(ProjectLink::from("project_1"))),
    ] {
        assert_eq!(
            catalog
                .project_of(&an_idea(idea))
                .await
                .expect("asking should succeed"),
            project,
            "{idea}"
        );
    }
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many_of_each_kind(
    catalog: &impl AppearanceCatalog,
) {
    for (idea, section, laid) in [
        ("idea_newest", "section_newest", LAID + Duration::SECOND * 2),
        ("idea_oldest", "section_oldest", LAID),
        ("idea_older", "section_older", LAID + Duration::SECOND),
    ] {
        catalog
            .remember_project(&an_idea(idea), &ProjectLink::from("project_1"))
            .await
            .expect("remembering should succeed");
        catalog
            .forget_subject(&an_idea(idea), laid)
            .await
            .expect("forgetting should succeed");
        catalog
            .forget_place(&a_section(section), laid)
            .await
            .expect("forgetting should succeed");
    }
    let before = LAID + Duration::DAY;

    let forgotten = catalog
        .forget_tombstones_laid_before(before, 2)
        .await
        .expect("forgetting tombstones should succeed");

    assert_eq!(forgotten, 4, "two of each kind");
    for (idea, project) in [
        ("idea_newest", Some(ProjectLink::from("project_1"))),
        ("idea_oldest", None),
        ("idea_older", None),
    ] {
        assert_eq!(
            catalog
                .project_of(&an_idea(idea))
                .await
                .expect("asking should succeed"),
            project,
            "{idea}"
        );
    }
    for idea in ["idea_newest", "idea_oldest", "idea_older"] {
        remembered(catalog, idea, &[a_scene("scene_1")]).await;
    }
    remembered(
        catalog,
        "idea_here",
        &[
            a_section("section_newest"),
            a_section("section_oldest"),
            a_section("section_older"),
        ],
    )
    .await;
    assert!(places_of(catalog, "idea_newest").await.is_empty());
    assert_eq!(
        places_of(catalog, "idea_oldest").await,
        vec![a_scene("scene_1")]
    );
    assert_eq!(
        places_of(catalog, "idea_older").await,
        vec![a_scene("scene_1")]
    );
    assert_eq!(
        places_of(catalog, "idea_here").await,
        vec![a_section("section_older"), a_section("section_oldest")]
    );
    for left in [2, 0] {
        assert_eq!(
            catalog
                .forget_tombstones_laid_before(before, 2)
                .await
                .expect("forgetting tombstones should succeed"),
            left
        );
    }
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
        $crate::suite::conformance_case!($workbench, an_idea_nobody_placed_appears_nowhere);
        $crate::suite::conformance_case!(
            $workbench,
            an_idea_s_project_is_kept_and_none_known_is_none
        );
        $crate::suite::conformance_case!(
            $workbench,
            remembering_an_idea_s_project_again_keeps_one_the_last
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_discarded_idea_s_project_outlives_it_as_its_tombstone_does
        );
        $crate::suite::conformance_case!($workbench, an_idea_appears_wherever_it_was_remembered);
        $crate::suite::conformance_case!(
            $workbench,
            places_come_back_in_one_order_whatever_order_they_were_remembered_in
        );
        $crate::suite::conformance_case!(
            $workbench,
            remembering_the_same_place_twice_is_one_appearance
        );
        $crate::suite::conformance_case!($workbench, forgetting_one_place_leaves_the_others);
        $crate::suite::conformance_case!(
            $workbench,
            a_late_remembering_does_not_undo_a_later_forgetting
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_late_forgetting_does_not_undo_a_later_remembering
        );
        $crate::suite::conformance_case!($workbench, nothing_comes_back_to_a_forgotten_place);
        $crate::suite::conformance_case!($workbench, a_forgotten_idea_comes_back_nowhere);
        $crate::suite::conformance_case!($workbench, forgetting_an_idea_leaves_every_other_idea);
        $crate::suite::conformance_case!($workbench, forgetting_a_place_reaches_every_idea_in_it);
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_a_place_says_which_ideas_appeared_there
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_section_and_a_scene_sharing_an_id_are_different_places
        );
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_what_was_never_remembered_is_harmless
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_laid_before_a_moment_are_forgotten_and_the_later_kept
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_tombstone_is_laid_once_when_what_it_marks_first_went
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_long_gone_idea_s_project_is_forgotten_with_its_tombstone
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many_of_each_kind
        );
    };
}

pub(crate) use {conformance_case, conformance_tests};
