use readings_core::{
    BookOutline, CatalogError, OutlineCatalog, OutlinedSection, ProjectLink, SceneLink, SectionLink,
};
use time::OffsetDateTime;
use transacting::Tried;

use crate::suite::Committing;

impl<C: OutlineCatalog> Committing<'_, C> {
    pub async fn keep_outline(
        &self,
        project: &ProjectLink,
        sections: &[OutlinedSection],
        version: u64,
    ) -> Result<bool, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .catalog
                .keep_outline(transaction, project, sections, version)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }

    pub async fn keep_title(
        &self,
        project: &ProjectLink,
        title: &str,
        version: u64,
    ) -> Result<bool, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .catalog
                .keep_title(transaction, project, title, version)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }

    pub async fn forget_book(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget_book(transaction, project, at)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }

    pub async fn forget_book_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(forgotten)
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }
}

fn a_project(named: &str) -> ProjectLink {
    ProjectLink::from(named)
}

fn a_section(named: &str, depth: u8, scenes: &[&str]) -> OutlinedSection {
    OutlinedSection {
        section: SectionLink::from(named),
        depth,
        title: format!("Title of {named}"),
        scenes: scenes.iter().map(|scene| SceneLink::from(*scene)).collect(),
    }
}

fn morning() -> Vec<OutlinedSection> {
    vec![
        a_section("section_morning", 0, &["scene_harbour", "scene_wall"]),
        a_section("section_market", 1, &["scene_stalls"]),
    ]
}

fn evening() -> Vec<OutlinedSection> {
    vec![a_section("section_evening", 0, &["scene_lamps"])]
}

async fn outline_kept(
    catalog: &Committing<'_, impl OutlineCatalog>,
    project: &str,
    sections: &[OutlinedSection],
    version: u64,
) {
    catalog
        .keep_outline(&a_project(project), sections, version)
        .await
        .expect("keeping should succeed");
}

async fn title_kept(
    catalog: &Committing<'_, impl OutlineCatalog>,
    project: &str,
    title: &str,
    version: u64,
) {
    catalog
        .keep_title(&a_project(project), title, version)
        .await
        .expect("keeping should succeed");
}

async fn book_forgotten(
    catalog: &Committing<'_, impl OutlineCatalog>,
    project: &str,
    at: OffsetDateTime,
) {
    catalog
        .forget_book(&a_project(project), at)
        .await
        .expect("forgetting should succeed");
}

async fn outline_of(
    catalog: &Committing<'_, impl OutlineCatalog>,
    project: &str,
) -> Option<BookOutline> {
    catalog
        .outline_of(&a_project(project))
        .await
        .expect("asking should succeed")
}

fn at(seconds: i64) -> OffsetDateTime {
    clock::at(seconds)
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: a book kept
    title_kept(catalog, "project_kept", "The Loom", 1).await;
    outline_kept(catalog, "project_kept", &morning(), 1).await;

    // when: in a transaction that is dropped, another book is kept and the first forgotten
    {
        let mut transaction = catalog.begun().await;
        catalog
            .uncommitted()
            .keep_title(&mut transaction, &a_project("project_dropped"), "Gone", 1)
            .await
            .expect("keeping should succeed");
        catalog
            .uncommitted()
            .keep_outline(
                &mut transaction,
                &a_project("project_dropped"),
                &evening(),
                1,
            )
            .await
            .expect("keeping should succeed");
        catalog
            .uncommitted()
            .forget_book(&mut transaction, &a_project("project_kept"), at(1_000))
            .await
            .expect("forgetting should succeed");
    }

    // then: only what was there before stands
    assert_eq!(
        outline_of(catalog, "project_kept").await,
        Some(BookOutline {
            title: "The Loom".to_owned(),
            sections: morning(),
        })
    );
    assert_eq!(outline_of(catalog, "project_dropped").await, None);
}

pub async fn a_book_never_told_is_not_there(catalog: &Committing<'_, impl OutlineCatalog>) {
    // expect: no book for a project nobody told of
    assert_eq!(outline_of(catalog, "project_untold").await, None);
}

pub async fn a_book_is_its_title_and_its_sections_in_their_order(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: a project's title and outline, and another project's
    title_kept(catalog, "project_loom", "The Loom", 1).await;
    outline_kept(catalog, "project_loom", &morning(), 3).await;
    title_kept(catalog, "project_other", "Another", 1).await;
    outline_kept(catalog, "project_other", &evening(), 1).await;

    // when: the book is asked for
    let book = outline_of(catalog, "project_loom").await;

    // then: it is its own title and sections, as they were kept
    assert_eq!(
        book,
        Some(BookOutline {
            title: "The Loom".to_owned(),
            sections: morning(),
        })
    );
}

pub async fn a_book_with_only_a_title_has_no_sections_and_one_with_only_an_outline_no_title(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: one project of which only the title was told, and one of which only the outline
    title_kept(catalog, "project_titled", "The Loom", 1).await;
    outline_kept(catalog, "project_outlined", &morning(), 1).await;

    // expect: each is there, with what it lacks left empty
    assert_eq!(
        outline_of(catalog, "project_titled").await,
        Some(BookOutline {
            title: "The Loom".to_owned(),
            sections: vec![],
        })
    );
    assert_eq!(
        outline_of(catalog, "project_outlined").await,
        Some(BookOutline {
            title: String::new(),
            sections: morning(),
        })
    );
}

pub async fn a_late_outline_does_not_undo_a_newer_one(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: an outline kept at a version
    outline_kept(catalog, "project_loom", &morning(), 4).await;

    // when: one told earlier arrives late, and another at the same version
    outline_kept(catalog, "project_loom", &evening(), 3).await;
    outline_kept(catalog, "project_loom", &evening(), 4).await;

    // then: the newer one stands
    assert_eq!(
        outline_of(catalog, "project_loom")
            .await
            .map(|book| book.sections),
        Some(morning())
    );
}

pub async fn a_newer_outline_takes_the_place_of_an_older_one(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: an outline kept
    outline_kept(catalog, "project_loom", &morning(), 3).await;

    // when: a newer one is kept
    outline_kept(catalog, "project_loom", &evening(), 4).await;

    // then: it is the book's outline
    assert_eq!(
        outline_of(catalog, "project_loom")
            .await
            .map(|book| book.sections),
        Some(evening())
    );
}

pub async fn a_late_title_does_not_undo_a_newer_one(catalog: &Committing<'_, impl OutlineCatalog>) {
    // given: a title kept, and a newer one
    title_kept(catalog, "project_loom", "The Loom", 1).await;
    title_kept(catalog, "project_loom", "The Loom, Again", 2).await;

    // when: one told earlier arrives late, and another at the same version
    title_kept(catalog, "project_loom", "The Loom", 1).await;
    title_kept(catalog, "project_loom", "Told twice", 2).await;

    // then: the newer one stands
    assert_eq!(
        outline_of(catalog, "project_loom")
            .await
            .map(|book| book.title),
        Some("The Loom, Again".to_owned())
    );
}

pub async fn the_title_and_the_outline_each_count_their_own_versions(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: an outline kept at a high version
    outline_kept(catalog, "project_loom", &morning(), 9).await;

    // when: the project's first title is kept, at a low one
    title_kept(catalog, "project_loom", "The Loom", 1).await;

    // then: both stand, as neither is older than the other's
    assert_eq!(
        outline_of(catalog, "project_loom").await,
        Some(BookOutline {
            title: "The Loom".to_owned(),
            sections: morning(),
        })
    );
}

pub async fn keeping_says_whether_it_kept(catalog: &Committing<'_, impl OutlineCatalog>) {
    // expect: a first outline and title are kept, newer ones too, and late ones not
    let project = a_project("project_loom");
    for (version, kept) in [(1, true), (2, true), (1, false)] {
        assert_eq!(
            catalog
                .keep_outline(&project, &morning(), version)
                .await
                .expect("keeping should succeed"),
            kept,
            "outline at {version}"
        );
        assert_eq!(
            catalog
                .keep_title(&project, "The Loom", version)
                .await
                .expect("keeping should succeed"),
            kept,
            "title at {version}"
        );
    }
}

pub async fn a_forgotten_book_is_not_there_and_nothing_late_brings_it_back(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: a book kept, then forgotten
    title_kept(catalog, "project_loom", "The Loom", 1).await;
    outline_kept(catalog, "project_loom", &morning(), 1).await;
    book_forgotten(catalog, "project_loom", at(1_000)).await;

    // when: a newer outline and title of it arrive late
    let outline = catalog
        .keep_outline(&a_project("project_loom"), &evening(), 9)
        .await
        .expect("keeping should succeed");
    let title = catalog
        .keep_title(&a_project("project_loom"), "Too late", 9)
        .await
        .expect("keeping should succeed");

    // then: neither is kept, and the book is not there
    assert!(!outline);
    assert!(!title);
    assert_eq!(outline_of(catalog, "project_loom").await, None);
}

pub async fn a_book_s_tombstone_is_laid_once_when_it_first_went(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: a book forgotten, and forgotten again later
    book_forgotten(catalog, "project_loom", at(1_000)).await;
    book_forgotten(catalog, "project_loom", at(5_000)).await;

    // when: tombstones laid before the second time are forgotten
    let forgotten = catalog
        .forget_book_tombstones_laid_before(at(2_000), 10)
        .await
        .expect("forgetting should succeed");

    // then: the tombstone goes, as it was laid when the book first went
    assert_eq!(forgotten, 1);
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl OutlineCatalog>,
) {
    // given: three tombstones, laid one after the other, and a young one
    for (project, laid) in [
        ("project_third", 3_000),
        ("project_first", 1_000),
        ("project_second", 2_000),
        ("project_young", 9_000),
    ] {
        book_forgotten(catalog, project, at(laid)).await;
    }

    // when: at most two of those laid before the young one are forgotten
    let forgotten = catalog
        .forget_book_tombstones_laid_before(at(5_000), 2)
        .await
        .expect("forgetting should succeed");

    // then: two went, the oldest
    assert_eq!(forgotten, 2);
    // and: the third still keeps a late title out, and the two oldest no longer do
    for (project, kept_out) in [
        ("project_first", false),
        ("project_second", false),
        ("project_third", true),
        ("project_young", true),
    ] {
        let kept = catalog
            .keep_title(&a_project(project), "Late", 1)
            .await
            .expect("keeping should succeed");
        assert_eq!(kept, !kept_out, "{project}");
    }
}

macro_rules! outline_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::outline_suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! outline_conformance_tests {
    ($workbench:ty) => {
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_book_never_told_is_not_there
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_book_is_its_title_and_its_sections_in_their_order
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_book_with_only_a_title_has_no_sections_and_one_with_only_an_outline_no_title
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_late_outline_does_not_undo_a_newer_one
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_newer_outline_takes_the_place_of_an_older_one
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_late_title_does_not_undo_a_newer_one
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            the_title_and_the_outline_each_count_their_own_versions
        );
        $crate::outline_suite::outline_conformance_case!($workbench, keeping_says_whether_it_kept);
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_forgotten_book_is_not_there_and_nothing_late_brings_it_back
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            a_book_s_tombstone_is_laid_once_when_it_first_went
        );
        $crate::outline_suite::outline_conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
    };
}

pub(crate) use {outline_conformance_case, outline_conformance_tests};
