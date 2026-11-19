use std::collections::HashMap;
use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use async_trait::async_trait;
use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};

use crate::catalog::{
    AccountNameCatalog, BookOutline, CatalogError, OutlineCatalog, OutlinedSection,
    ReadingCopyCatalog, ReadingCopyContents, ReadingCopySummary, SceneTextCatalog,
};
use crate::contents::{Block, Contents, Part, Run};
use crate::error::ReadingsError;
use crate::following::NobodyFollows;
use crate::id::ReadingCopyId;
use crate::link::{ProjectLink, SceneLink, SectionLink};
use crate::readings::ReadingsCore;

/// No reading copies, as these cases are about the live book.
struct NoCopies;

#[async_trait]
impl ReadingCopyCatalog for NoCopies {
    async fn remember(
        &self,
        _transaction: &mut Transaction,
        _summary: &ReadingCopySummary,
    ) -> Result<(), CatalogError> {
        Ok(())
    }

    async fn forget(
        &self,
        _transaction: &mut Transaction,
        _copy: &ReadingCopyId,
    ) -> Result<(), CatalogError> {
        Ok(())
    }

    async fn copies_of(
        &self,
        _project: &ProjectLink,
    ) -> Result<Vec<ReadingCopySummary>, CatalogError> {
        Ok(vec![])
    }
}

/// No names of accounts, as these cases are about the live book.
struct NoNames;

#[async_trait]
impl AccountNameCatalog for NoNames {
    async fn remember_name(
        &self,
        _transaction: &mut Transaction,
        _account: &AccountId,
        _name: &str,
    ) -> Result<(), CatalogError> {
        Ok(())
    }

    async fn forget_name(
        &self,
        _transaction: &mut Transaction,
        _account: &AccountId,
        _at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        _transaction: &mut Transaction,
        _before: OffsetDateTime,
        _at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(0)
    }

    async fn names_of(
        &self,
        _accounts: &[AccountId],
    ) -> Result<HashMap<AccountId, String>, CatalogError> {
        Ok(HashMap::new())
    }
}

/// No reading copies' contents, as these cases are about the live book.
struct NoContents;

#[async_trait]
impl ReadingCopyContents for NoContents {
    async fn keep(
        &self,
        _transaction: &mut Transaction,
        _copy: &ReadingCopyId,
        _contents: &Contents,
    ) -> Result<(), CatalogError> {
        Ok(())
    }

    async fn contents_of(&self, _copy: &ReadingCopyId) -> Result<Option<Contents>, CatalogError> {
        Ok(None)
    }

    async fn forget(
        &self,
        _transaction: &mut Transaction,
        _copy: &ReadingCopyId,
    ) -> Result<(), CatalogError> {
        Ok(())
    }
}

struct Outlines(HashMap<ProjectLink, BookOutline>);

#[async_trait]
impl OutlineCatalog for Outlines {
    async fn keep_outline(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _sections: &[OutlinedSection],
        _version: u64,
    ) -> Result<bool, CatalogError> {
        Ok(false)
    }

    async fn keep_title(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _title: &str,
        _version: u64,
    ) -> Result<bool, CatalogError> {
        Ok(false)
    }

    async fn forget_book(
        &self,
        _transaction: &mut Transaction,
        _project: &ProjectLink,
        _at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        _transaction: &mut Transaction,
        _before: OffsetDateTime,
        _at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(0)
    }

    async fn outline_of(&self, project: &ProjectLink) -> Result<Option<BookOutline>, CatalogError> {
        Ok(self.0.get(project).cloned())
    }
}

struct Texts(HashMap<SceneLink, Vec<Block>>);

#[async_trait]
impl SceneTextCatalog for Texts {
    async fn keep(
        &self,
        _transaction: &mut Transaction,
        _scene: &SceneLink,
        _blocks: &[Block],
        _version: u64,
    ) -> Result<bool, CatalogError> {
        Ok(false)
    }

    async fn forget_scene(
        &self,
        _transaction: &mut Transaction,
        _scene: &SceneLink,
        _at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        _transaction: &mut Transaction,
        _before: OffsetDateTime,
        _at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(0)
    }

    async fn texts_of(
        &self,
        scenes: &[SceneLink],
    ) -> Result<HashMap<SceneLink, Vec<Block>>, CatalogError> {
        Ok(scenes
            .iter()
            .filter_map(|scene| self.0.get(scene).map(|text| (scene.clone(), text.clone())))
            .collect())
    }
}

fn section(id: &str, depth: u8, scenes: &[&str]) -> OutlinedSection {
    OutlinedSection {
        section: SectionLink::from(id),
        depth,
        title: format!("Title of {id}"),
        scenes: scenes.iter().map(|scene| SceneLink::from(*scene)).collect(),
    }
}

fn text(words: &str) -> Vec<Block> {
    vec![Block::Paragraph(vec![Run::plain(words)])]
}

fn readings(sections: Vec<OutlinedSection>, texts: &[(&str, &str)]) -> ReadingsCore {
    let outline = BookOutline {
        title: "The Loom".to_owned(),
        sections,
    };

    ReadingsCore::new(
        Arc::new(Outlines(HashMap::from([(
            ProjectLink::from("project_loom"),
            outline,
        )]))),
        Arc::new(Texts(
            texts
                .iter()
                .map(|(scene, words)| (SceneLink::from(*scene), text(words)))
                .collect(),
        )),
        Arc::new(NoCopies),
        Arc::new(NoNames),
        Arc::new(NoContents),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(NobodyFollows),
        Arc::new(InMemoryTransactions::new()),
        KeptAccess::new(Arc::new(InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    )
}

fn shape(parts: &[Part]) -> Vec<String> {
    parts
        .iter()
        .map(|part| match part {
            Part::Section { section, depth, .. } => {
                format!("{}{section}", "  ".repeat(*depth as usize))
            }
            Part::Scene { scene, .. } => format!("- {scene}"),
        })
        .collect()
}

#[tokio::test]
async fn the_book_lays_out_each_section_then_the_scenes_it_holds() {
    // given: a book of a part with two scenes and a nested chapter with one
    let readings = readings(
        vec![
            section("part_one", 0, &["scene_harbour", "scene_wall"]),
            section("chapter_market", 1, &["scene_stalls"]),
        ],
        &[
            ("scene_harbour", "The boats came in."),
            ("scene_wall", "Hedy counted them."),
            ("scene_stalls", "Fresh today."),
        ],
    );

    // when: the live book is asked for
    let book = readings
        .live_book("project_loom", &Agent::Local)
        .await
        .expect("the book should be put together");

    // then: it carries the outline's title
    assert_eq!(book.title, "The Loom");
    // and: each section comes before its scenes, at its depth, in reading order
    assert_eq!(
        shape(&book.parts),
        [
            "part_one",
            "- scene_harbour",
            "- scene_wall",
            "  chapter_market",
            "- scene_stalls",
        ]
    );
    // and: each scene holds its own text
    let Part::Scene { blocks, .. } = &book.parts[2] else {
        panic!("the third part is a scene");
    };
    assert_eq!(blocks, &text("Hedy counted them."));
}

#[tokio::test]
async fn a_section_that_holds_no_scene_is_its_heading_alone() {
    // given: a book whose first section holds nothing yet
    let readings = readings(
        vec![
            section("part_one", 0, &[]),
            section("part_two", 0, &["scene_harbour"]),
        ],
        &[("scene_harbour", "The boats came in.")],
    );

    // when: the live book is asked for
    let book = readings
        .live_book("project_loom", &Agent::Local)
        .await
        .expect("the book should be put together");

    // then: the empty section is a heading followed by the next
    assert_eq!(
        shape(&book.parts),
        ["part_one", "part_two", "- scene_harbour"]
    );
}

#[tokio::test]
async fn a_scene_whose_text_is_not_kept_yet_is_there_with_nothing_in_it() {
    // given: a book of two scenes, the text of one not kept yet
    let readings = readings(
        vec![section("part_one", 0, &["scene_harbour", "scene_new"])],
        &[("scene_harbour", "The boats came in.")],
    );

    // when: the live book is asked for
    let book = readings
        .live_book("project_loom", &Agent::Local)
        .await
        .expect("the book should be put together");

    // then: both scenes are in it, so a follower knows every scene the book holds
    assert_eq!(
        shape(&book.parts),
        ["part_one", "- scene_harbour", "- scene_new"]
    );
    // and: the one not kept yet has nothing in it
    assert!(matches!(
        &book.parts[2],
        Part::Scene { blocks, .. } if blocks.is_empty()
    ));
}

#[tokio::test]
async fn a_project_without_an_outline_is_not_there() {
    // given: the book of another project
    let readings = readings(vec![section("part_one", 0, &[])], &[]);

    // when: the live book of a project with no outline is asked for
    let refused = readings
        .live_book("project_unknown", &Agent::Local)
        .await
        .expect_err("a project without an outline has no book");

    // then: the project is not there
    assert!(matches!(refused, ReadingsError::ProjectNotThere));
}
