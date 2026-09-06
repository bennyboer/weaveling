use outline_core::{OutlineCatalog, OutlineId, OutlineSummary, PieceLink, ProjectLink};
use time::OffsetDateTime;

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

pub async fn a_piece_nobody_placed_is_in_no_outline(catalog: &impl OutlineCatalog) {
    let found = catalog
        .outlines_holding(&PieceLink::from("piece_loose"))
        .await
        .expect("looking should succeed");

    assert!(
        found.is_empty(),
        "a piece in the pool and not in the book is not an error, it is simply not in the book yet"
    );
}

pub async fn an_attached_piece_names_the_outline_holding_it(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));

    catalog
        .holds(outline, &[PieceLink::from("piece_1")])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .outlines_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed"),
        vec![outline]
    );
}

pub async fn a_piece_may_sit_in_more_than_one_outline(catalog: &impl OutlineCatalog) {
    let earliest = OutlineId::generate(at(1_000));
    let latest = OutlineId::generate(at(2_000));
    for outline in [latest, earliest] {
        catalog
            .holds(outline, &[PieceLink::from("piece_1")])
            .await
            .expect("indexing should succeed");
    }

    assert_eq!(
        catalog
            .outlines_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed"),
        vec![earliest, latest],
        "one piece sits in at most one section, but the model allows a project several outlines"
    );
}

pub async fn what_an_outline_holds_is_replaced_not_added_to(catalog: &impl OutlineCatalog) {
    let outline = OutlineId::generate(at(1_000));
    catalog
        .holds(outline, &[PieceLink::from("piece_1")])
        .await
        .expect("indexing should succeed");

    catalog
        .holds(outline, &[PieceLink::from("piece_2")])
        .await
        .expect("indexing again should succeed");

    assert!(
        catalog
            .outlines_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "the projector writes the whole set, so a detached piece falls out of the index"
    );
    assert_eq!(
        catalog
            .outlines_holding(&PieceLink::from("piece_2"))
            .await
            .expect("looking should succeed"),
        vec![outline]
    );
}

pub async fn one_outline_letting_a_piece_go_leaves_the_others_holding_it(
    catalog: &impl OutlineCatalog,
) {
    let keeping = OutlineId::generate(at(1_000));
    let dropping = OutlineId::generate(at(2_000));
    for outline in [keeping, dropping] {
        catalog
            .holds(outline, &[PieceLink::from("piece_1")])
            .await
            .expect("indexing should succeed");
    }

    catalog
        .holds(dropping, &[])
        .await
        .expect("indexing should succeed");

    assert_eq!(
        catalog
            .outlines_holding(&PieceLink::from("piece_1"))
            .await
            .expect("looking should succeed"),
        vec![keeping],
        "an index kept in both directions must not forget the outlines that still hold it"
    );
}

#[macro_export]
macro_rules! catalog_conformance_case {
    ($make_catalog:expr, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            $crate::suite::$case(&$make_catalog).await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($make_catalog:expr) => {
        $crate::catalog_conformance_case!(
            $make_catalog,
            a_remembered_outline_is_listed_in_its_project
        );
        $crate::catalog_conformance_case!(
            $make_catalog,
            a_project_that_never_opened_an_outline_lists_nothing
        );
        $crate::catalog_conformance_case!($make_catalog, outlines_of_other_projects_are_not_listed);
        $crate::catalog_conformance_case!(
            $make_catalog,
            remembering_the_same_outline_again_replaces_what_was_there
        );
        $crate::catalog_conformance_case!(
            $make_catalog,
            a_project_lists_its_outlines_in_a_settled_order
        );
        $crate::catalog_conformance_case!($make_catalog, a_piece_nobody_placed_is_in_no_outline);
        $crate::catalog_conformance_case!(
            $make_catalog,
            an_attached_piece_names_the_outline_holding_it
        );
        $crate::catalog_conformance_case!($make_catalog, a_piece_may_sit_in_more_than_one_outline);
        $crate::catalog_conformance_case!(
            $make_catalog,
            what_an_outline_holds_is_replaced_not_added_to
        );
        $crate::catalog_conformance_case!(
            $make_catalog,
            one_outline_letting_a_piece_go_leaves_the_others_holding_it
        );
    };
}
