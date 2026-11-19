use readings_core::{
    Block, CatalogError, Contents, Marks, Part, ProjectLink, ReadingCopyContents, ReadingCopyId,
    Run, SceneLink, SectionLink,
};
use transacting::Tried;

use crate::suite::Committing;

impl<C: ReadingCopyContents> Committing<'_, C> {
    pub async fn keep_contents(
        &self,
        copy: &ReadingCopyId,
        contents: &Contents,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .keep(transaction, copy, contents)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }

    pub async fn forget_contents(&self, copy: &ReadingCopyId) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget(transaction, copy)
                .await
                .map_err(Tried::Refused)?;

            Ok(())
        })
        .map_err(|tried: Tried<_>| tried.answered(crate::suite::unreachable))
    }
}

fn a_copy(made_at: i64) -> ReadingCopyId {
    ReadingCopyId::generate(clock::at(made_at))
}

/// A work of two sections, the second nested in the first, with a scene of marked words and one with nothing written.
fn the_loom() -> Contents {
    Contents {
        project: ProjectLink::from("project_loom"),
        title: "The Loom".to_owned(),
        parts: vec![
            Part::Section {
                section: SectionLink::from("section_morning"),
                depth: 0,
                title: "Morning".to_owned(),
            },
            Part::Scene {
                scene: SceneLink::from("scene_harbour"),
                blocks: vec![Block::Paragraph(vec![
                    Run::plain("The boats came in "),
                    Run::Text {
                        text: "one by one".to_owned(),
                        marks: Marks {
                            emphasis: true,
                            ..Marks::default()
                        },
                    },
                ])],
            },
            Part::Section {
                section: SectionLink::from("section_market"),
                depth: 1,
                title: "The market".to_owned(),
            },
            Part::Scene {
                scene: SceneLink::from("scene_stalls"),
                blocks: vec![],
            },
        ],
    }
}

fn another_work() -> Contents {
    Contents {
        project: ProjectLink::from("project_other"),
        title: "Another".to_owned(),
        parts: vec![],
    }
}

async fn kept(
    catalog: &Committing<'_, impl ReadingCopyContents>,
    copy: &ReadingCopyId,
    contents: &Contents,
) {
    catalog
        .keep_contents(copy, contents)
        .await
        .expect("keeping should succeed");
}

async fn contents_of(
    catalog: &Committing<'_, impl ReadingCopyContents>,
    copy: &ReadingCopyId,
) -> Option<Contents> {
    catalog
        .contents_of(copy)
        .await
        .expect("asking should succeed")
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl ReadingCopyContents>,
) {
    // given: one copy's contents kept
    let staying = a_copy(10);
    kept(catalog, &staying, &the_loom()).await;

    // when: in a transaction that is dropped, another copy's contents are kept and the first forgotten
    let dropped = a_copy(20);
    {
        let mut transaction = catalog.begun().await;
        catalog
            .uncommitted()
            .keep(&mut transaction, &dropped, &another_work())
            .await
            .expect("keeping should succeed");
        catalog
            .uncommitted()
            .forget(&mut transaction, &staying)
            .await
            .expect("forgetting should succeed");
    }

    // then: only what was there before stands
    assert_eq!(contents_of(catalog, &staying).await, Some(the_loom()));
    assert_eq!(contents_of(catalog, &dropped).await, None);
}

pub async fn a_copy_s_contents_read_back_as_they_were_kept(
    catalog: &Committing<'_, impl ReadingCopyContents>,
) {
    // given: two copies, each of a different work
    let loom = a_copy(10);
    let other = a_copy(20);
    kept(catalog, &loom, &the_loom()).await;
    kept(catalog, &other, &another_work()).await;

    // expect: each reads back as it was kept, with its project, its title and its parts in order
    assert_eq!(contents_of(catalog, &loom).await, Some(the_loom()));
    assert_eq!(contents_of(catalog, &other).await, Some(another_work()));
    // and: a copy never kept holds nothing
    assert_eq!(contents_of(catalog, &a_copy(30)).await, None);
}

pub async fn a_copy_s_contents_never_change_once_kept(
    catalog: &Committing<'_, impl ReadingCopyContents>,
) {
    // given: a copy's contents kept
    let copy = a_copy(10);
    kept(catalog, &copy, &the_loom()).await;

    // when: other contents are kept for it again
    kept(catalog, &copy, &another_work()).await;

    // then: it still holds what it was first kept with
    assert_eq!(contents_of(catalog, &copy).await, Some(the_loom()));
}

pub async fn a_copy_s_contents_forgotten_are_gone(
    catalog: &Committing<'_, impl ReadingCopyContents>,
) {
    // given: two copies' contents kept
    let forgotten = a_copy(10);
    let staying = a_copy(20);
    kept(catalog, &forgotten, &the_loom()).await;
    kept(catalog, &staying, &the_loom()).await;

    // when: one is forgotten, and a copy never kept as well
    for copy in [forgotten, a_copy(30)] {
        catalog
            .forget_contents(&copy)
            .await
            .expect("forgetting should succeed");
    }

    // then: only the other holds anything
    assert_eq!(contents_of(catalog, &forgotten).await, None);
    assert_eq!(contents_of(catalog, &staying).await, Some(the_loom()));
}

macro_rules! contents_conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::contents_suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! contents_conformance_tests {
    ($workbench:ty) => {
        $crate::contents_suite::contents_conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::contents_suite::contents_conformance_case!(
            $workbench,
            a_copy_s_contents_read_back_as_they_were_kept
        );
        $crate::contents_suite::contents_conformance_case!(
            $workbench,
            a_copy_s_contents_never_change_once_kept
        );
        $crate::contents_suite::contents_conformance_case!(
            $workbench,
            a_copy_s_contents_forgotten_are_gone
        );
    };
}

pub(crate) use {contents_conformance_case, contents_conformance_tests};
