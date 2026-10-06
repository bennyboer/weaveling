use appearances_core::{AppearanceCatalog, IdeaLink, PassageLink, Place, SectionLink, Subject};

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

fn a_passage(named: &str) -> Place {
    Place::Passage(PassageLink::from(named))
}

async fn remembered(catalog: &impl AppearanceCatalog, idea: &str, places: &[Place]) {
    for place in places {
        catalog
            .remember(&an_idea(idea), place)
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
        &[a_passage("passage_1"), a_section("section_1")],
    )
    .await;

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_passage("passage_1"), a_section("section_1")]
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
            a_passage("passage_1"),
            a_section("section_1"),
        ],
    )
    .await;

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![
            a_passage("passage_1"),
            a_section("section_1"),
            a_section("section_2"),
        ],
        "remembered shuffled on purpose: passages come back before sections, each by id, \
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
        &[a_section("section_1"), a_passage("passage_1")],
    )
    .await;

    catalog
        .forget(&an_idea("idea_1"), &a_section("section_1"))
        .await
        .expect("forgetting should succeed");

    assert_eq!(
        places_of(catalog, "idea_1").await,
        vec![a_passage("passage_1")]
    );
}

pub async fn forgetting_an_idea_leaves_every_other_idea(catalog: &impl AppearanceCatalog) {
    remembered(catalog, "idea_gone", &[a_section("section_1")]).await;
    remembered(catalog, "idea_kept", &[a_section("section_1")]).await;

    catalog
        .forget_subject(&an_idea("idea_gone"))
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
        .forget_place(&a_section("section_gone"))
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

pub async fn a_section_and_a_passage_sharing_an_id_are_different_places(
    catalog: &impl AppearanceCatalog,
) {
    remembered(
        catalog,
        "idea_1",
        &[a_section("same_1"), a_passage("same_1")],
    )
    .await;

    catalog
        .forget_place(&a_passage("same_1"))
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
        .forget(&an_idea("idea_1"), &a_section("section_1"))
        .await
        .expect("forgetting nothing is not a failure");
    catalog
        .forget_subject(&an_idea("idea_1"))
        .await
        .expect("forgetting nothing is not a failure");
    catalog
        .forget_place(&a_section("section_1"))
        .await
        .expect("forgetting nothing is not a failure");
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
        $crate::suite::conformance_case!($workbench, forgetting_an_idea_leaves_every_other_idea);
        $crate::suite::conformance_case!($workbench, forgetting_a_place_reaches_every_idea_in_it);
        $crate::suite::conformance_case!(
            $workbench,
            a_section_and_a_passage_sharing_an_id_are_different_places
        );
        $crate::suite::conformance_case!(
            $workbench,
            forgetting_what_was_never_remembered_is_harmless
        );
    };
}

pub(crate) use {conformance_case, conformance_tests};
