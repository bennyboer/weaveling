use std::collections::HashMap;
use std::ops::Deref;

use readings_core::{Block, CatalogError, Run, SceneLink, SceneTextCatalog};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions, Tried};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Catalog: SceneTextCatalog;

    async fn setup() -> Self;

    fn catalog(&self) -> &Self::Catalog;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Catalog> {
        Committing {
            catalog: self.catalog(),
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

impl<C: SceneTextCatalog> Committing<'_, C> {
    pub async fn begun(&self) -> Transaction {
        self.transactions
            .begin()
            .await
            .expect("beginning should succeed")
    }

    pub fn uncommitted(&self) -> &C {
        self.catalog
    }

    pub async fn keep(
        &self,
        scene: &SceneLink,
        blocks: &[Block],
        version: u64,
    ) -> Result<bool, CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            let kept = self
                .catalog
                .keep(transaction, scene, blocks, version)
                .await
                .map_err(Tried::Refused)?;

            Ok(kept)
        })
        .map_err(|tried: Tried<_>| tried.answered(unreachable))
    }

    pub async fn forget_scene(
        &self,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::retrying!(self.transactions, |transaction| {
            self.catalog
                .forget_scene(transaction, scene, at)
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
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await
                .map_err(Tried::Refused)?;

            Ok(forgotten)
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

fn a_scene(named: &str) -> SceneLink {
    SceneLink::from(named)
}

fn saying(words: &str) -> Vec<Block> {
    vec![Block::Paragraph(vec![Run::plain(words)])]
}

async fn kept(
    catalog: &Committing<'_, impl SceneTextCatalog>,
    scene: &str,
    words: &str,
    version: u64,
) {
    catalog
        .keep(&a_scene(scene), &saying(words), version)
        .await
        .expect("keeping should succeed");
}

async fn texts_of(
    catalog: &Committing<'_, impl SceneTextCatalog>,
    scenes: &[&str],
) -> HashMap<SceneLink, Vec<Block>> {
    let scenes: Vec<SceneLink> = scenes.iter().map(|scene| a_scene(scene)).collect();

    catalog
        .texts_of(&scenes)
        .await
        .expect("asking should succeed")
}

fn at(seconds: i64) -> OffsetDateTime {
    clock::at(seconds)
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone(
    catalog: &Committing<'_, impl SceneTextCatalog>,
) {
    // given: a scene's text kept
    kept(catalog, "scene_kept", "The boats came in.", 1).await;

    // when: in a transaction that is dropped, another text is kept and the first scene forgotten
    {
        let mut transaction = catalog.begun().await;
        catalog
            .uncommitted()
            .keep(
                &mut transaction,
                &a_scene("scene_dropped"),
                &saying("Gone."),
                1,
            )
            .await
            .expect("keeping should succeed");
        catalog
            .uncommitted()
            .forget_scene(&mut transaction, &a_scene("scene_kept"), at(1_000))
            .await
            .expect("forgetting should succeed");
    }

    // then: only what was there before stands
    assert_eq!(
        texts_of(catalog, &["scene_kept", "scene_dropped"]).await,
        HashMap::from([(a_scene("scene_kept"), saying("The boats came in."))]),
        "nothing written in the dropped transaction stands"
    );
}

pub async fn a_scene_never_told_has_no_text(catalog: &Committing<'_, impl SceneTextCatalog>) {
    // when: the text of a scene nobody told is asked for
    let texts = texts_of(catalog, &["scene_untold"]).await;

    // then: there is none
    assert!(texts.is_empty());
}

pub async fn only_the_texts_asked_for_come_back(catalog: &Committing<'_, impl SceneTextCatalog>) {
    // given: the texts of three scenes
    kept(catalog, "scene_harbour", "The boats came in.", 1).await;
    kept(catalog, "scene_wall", "Hedy counted them.", 1).await;
    kept(catalog, "scene_stalls", "Fresh today.", 1).await;

    // when: two of them are asked for
    let texts = texts_of(catalog, &["scene_harbour", "scene_stalls"]).await;

    // then: those two come back, each with its own text
    assert_eq!(
        texts,
        HashMap::from([
            (a_scene("scene_harbour"), saying("The boats came in.")),
            (a_scene("scene_stalls"), saying("Fresh today.")),
        ])
    );
}

pub async fn a_newer_text_takes_the_place_of_an_older_one(
    catalog: &Committing<'_, impl SceneTextCatalog>,
) {
    // given: a scene's text kept
    kept(catalog, "scene_harbour", "The boats came in.", 3).await;

    // when: a text read at a later version is kept
    kept(catalog, "scene_harbour", "The boats came in at dawn.", 4).await;

    // then: the later one is the text
    assert_eq!(
        texts_of(catalog, &["scene_harbour"]).await,
        HashMap::from([(
            a_scene("scene_harbour"),
            saying("The boats came in at dawn.")
        )])
    );
}

pub async fn a_late_text_does_not_undo_a_newer_one(
    catalog: &Committing<'_, impl SceneTextCatalog>,
) {
    // given: a scene's text kept at a version
    kept(catalog, "scene_harbour", "The boats came in at dawn.", 4).await;

    // when: a text read at an earlier version arrives late, and another at the same version
    kept(catalog, "scene_harbour", "The boats came in.", 3).await;
    kept(catalog, "scene_harbour", "Told twice.", 4).await;

    // then: the newer one stands
    assert_eq!(
        texts_of(catalog, &["scene_harbour"]).await,
        HashMap::from([(
            a_scene("scene_harbour"),
            saying("The boats came in at dawn.")
        )])
    );
}

pub async fn keeping_says_whether_it_kept(catalog: &Committing<'_, impl SceneTextCatalog>) {
    // expect: a first text is kept, a newer one too, and a late one not
    assert!(
        catalog
            .keep(&a_scene("scene_harbour"), &saying("First."), 1)
            .await
            .expect("keeping should succeed")
    );
    assert!(
        catalog
            .keep(&a_scene("scene_harbour"), &saying("Second."), 2)
            .await
            .expect("keeping should succeed")
    );
    assert!(
        !catalog
            .keep(&a_scene("scene_harbour"), &saying("Late."), 1)
            .await
            .expect("keeping should succeed")
    );
}

pub async fn a_forgotten_scene_has_no_text(catalog: &Committing<'_, impl SceneTextCatalog>) {
    // given: a scene's text kept
    kept(catalog, "scene_harbour", "The boats came in.", 1).await;

    // when: the scene is forgotten
    catalog
        .forget_scene(&a_scene("scene_harbour"), at(1_000))
        .await
        .expect("forgetting should succeed");

    // then: it has no text
    assert!(texts_of(catalog, &["scene_harbour"]).await.is_empty());
}

pub async fn a_text_arriving_after_its_scene_was_forgotten_does_not_bring_it_back(
    catalog: &Committing<'_, impl SceneTextCatalog>,
) {
    // given: a forgotten scene
    catalog
        .forget_scene(&a_scene("scene_harbour"), at(1_000))
        .await
        .expect("forgetting should succeed");

    // when: a text of it arrives late
    let kept = catalog
        .keep(&a_scene("scene_harbour"), &saying("Too late."), 9)
        .await
        .expect("keeping should succeed");

    // then: it is not kept
    assert!(!kept);
    assert!(texts_of(catalog, &["scene_harbour"]).await.is_empty());
}

pub async fn a_scene_s_tombstone_is_laid_once_when_it_first_went(
    catalog: &Committing<'_, impl SceneTextCatalog>,
) {
    // given: a scene forgotten, and forgotten again later
    catalog
        .forget_scene(&a_scene("scene_harbour"), at(1_000))
        .await
        .expect("forgetting should succeed");
    catalog
        .forget_scene(&a_scene("scene_harbour"), at(5_000))
        .await
        .expect("forgetting should succeed");

    // when: tombstones laid before the second time are forgotten
    let forgotten = catalog
        .forget_tombstones_laid_before(at(2_000), 10)
        .await
        .expect("forgetting should succeed");

    // then: the tombstone goes, as it was laid when the scene first went
    assert_eq!(forgotten, 1);
}

pub async fn tombstones_are_forgotten_oldest_first_and_at_most_so_many(
    catalog: &Committing<'_, impl SceneTextCatalog>,
) {
    // given: three tombstones, laid one after the other, and a young one
    for (scene, laid) in [
        ("scene_third", 3_000),
        ("scene_first", 1_000),
        ("scene_second", 2_000),
        ("scene_young", 9_000),
    ] {
        catalog
            .forget_scene(&a_scene(scene), at(laid))
            .await
            .expect("forgetting should succeed");
    }

    // when: at most two of those laid before the young one are forgotten
    let forgotten = catalog
        .forget_tombstones_laid_before(at(5_000), 2)
        .await
        .expect("forgetting should succeed");

    // then: two went, the oldest
    assert_eq!(forgotten, 2);
    // and: the third still keeps a late text out, and the two oldest no longer do
    for (scene, kept_out) in [
        ("scene_first", false),
        ("scene_second", false),
        ("scene_third", true),
        ("scene_young", true),
    ] {
        let kept = catalog
            .keep(&a_scene(scene), &saying("Late."), 1)
            .await
            .expect("keeping should succeed");
        assert_eq!(kept, !kept_out, "{scene}");
    }
}

macro_rules! conformance_case {
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

macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::suite::conformance_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone
        );
        $crate::suite::conformance_case!($workbench, a_scene_never_told_has_no_text);
        $crate::suite::conformance_case!($workbench, only_the_texts_asked_for_come_back);
        $crate::suite::conformance_case!($workbench, a_newer_text_takes_the_place_of_an_older_one);
        $crate::suite::conformance_case!($workbench, a_late_text_does_not_undo_a_newer_one);
        $crate::suite::conformance_case!($workbench, keeping_says_whether_it_kept);
        $crate::suite::conformance_case!($workbench, a_forgotten_scene_has_no_text);
        $crate::suite::conformance_case!(
            $workbench,
            a_text_arriving_after_its_scene_was_forgotten_does_not_bring_it_back
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_scene_s_tombstone_is_laid_once_when_it_first_went
        );
        $crate::suite::conformance_case!(
            $workbench,
            tombstones_are_forgotten_oldest_first_and_at_most_so_many
        );
    };
}

pub(crate) use {conformance_case, conformance_tests};
