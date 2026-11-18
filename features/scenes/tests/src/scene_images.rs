use std::sync::Arc;
use std::time::Duration;

use authorizing::AllowingEverything;
use belonging::InMemoryBelongings;
use clock::FixedClock;
use eventsourcing::Agent;
use messaging::FakePublisher;
use outbox::{InMemoryOutbox, Outbox};
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_contract::{ASSETS_CHANGED, SceneAssetsChangedDTO};
use scenes_core::{FRAGMENT, SceneId, ScenesCore};
use scenes_store::InMemorySceneStore;
use scenes_sync::{LiveScenes, Message, Presence};
use transacting::InMemoryTransactions;
use yrs::types::xml::XmlOut;
use yrs::{
    Doc, ReadTxn, Transact, TransactionMut, Xml, XmlElementPrelim, XmlFragment, XmlFragmentRef,
    XmlTextPrelim,
};

const A_PROJECT: &str = "project_1";

pub(crate) struct Wired {
    pub(crate) scenes: ScenesCore,
    pub(crate) live: LiveScenes,
    pub(crate) outbox: Arc<InMemoryOutbox>,
    pub(crate) published: Arc<FakePublisher>,
}

pub(crate) fn a_workbench() -> Wired {
    let clock = Arc::new(FixedClock::new(clock::at(1_000)));
    let published = Arc::new(FakePublisher::default());
    let outbox = Arc::new(InMemoryOutbox::new(published.clone(), clock.clone()));
    let scenes = ScenesCore::new(
        Arc::new(InMemorySceneStore::new().enqueuing_to(
            outbox.clone(),
            clock.clone(),
            scenes_messaging::message_for,
        )),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
        crate::kept::heard_of_nobody(),
        Arc::new(AllowingEverything),
        clock,
    );

    Wired {
        live: LiveScenes::new(scenes.clone()),
        scenes,
        outbox,
        published,
    }
}

/// A writer's own copy of a scene, handing out each edit as an update.
pub(crate) struct Writer {
    doc: Doc,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self { doc: Doc::new() }
    }

    pub(crate) fn edited(
        &self,
        edit: impl FnOnce(&mut TransactionMut, &XmlFragmentRef),
    ) -> Vec<u8> {
        let before = self.doc.transact().state_vector();
        let fragment = self.doc.get_or_insert_xml_fragment(FRAGMENT);
        {
            let mut txn = self.doc.transact_mut();
            edit(&mut txn, &fragment);
        }

        self.doc.transact().encode_state_as_update_v1(&before)
    }

    pub(crate) fn a_paragraph(&self, saying: &str) -> Vec<u8> {
        self.edited(|txn, fragment| {
            let at = fragment.len(txn);
            let paragraph = fragment.insert(txn, at, XmlElementPrelim::empty("paragraph"));
            paragraph.insert(txn, 0, XmlTextPrelim::new(saying));
        })
    }

    fn a_figure(&self, asset: &str) -> Vec<u8> {
        self.edited(|txn, fragment| {
            let at = fragment.len(txn);
            let figure = fragment.insert(txn, at, XmlElementPrelim::empty("figure"));
            figure.insert_attribute(txn, "asset", asset);
            figure.insert_attribute(txn, "size", "full");
            figure.insert(txn, 0, XmlTextPrelim::new("The loom at dawn"));
        })
    }

    fn without_figures(&self) -> Vec<u8> {
        self.edited(|txn, fragment| {
            let figures: Vec<u32> = fragment
                .children(txn)
                .enumerate()
                .filter_map(|(at, node)| match node {
                    XmlOut::Element(element) if element.tag().as_ref() == "figure" => {
                        Some(at as u32)
                    }
                    _ => None,
                })
                .collect();
            for at in figures.into_iter().rev() {
                fragment.remove_range(txn, at, 1);
            }
        })
    }
}

impl Wired {
    pub(crate) async fn a_scene(&self) -> SceneId {
        self.scenes
            .create(A_PROJECT, &Agent::Local)
            .await
            .expect("should create")
            .id()
    }

    pub(crate) async fn writing(&self, present: &Presence, update: Vec<u8>) {
        present
            .receive(Message::JustHappened(update.clone()))
            .expect("should write");
        present.persist(&update).await.expect("should persist");
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    async fn assets_told(&self, id: SceneId) -> Vec<SceneAssetsChangedDTO> {
        self.outbox
            .deliver(100)
            .await
            .expect("delivering should succeed");

        self.published
            .heard()
            .into_iter()
            .filter(|message| message.routing.to_string() == ASSETS_CHANGED)
            .map(|message| {
                message
                    .payload_as::<SceneAssetsChangedDTO>()
                    .expect("the images read back as the contract says")
            })
            .filter(|told| told.scene == id.to_string())
            .collect()
    }

    pub(crate) async fn prose_version(&self, id: SceneId) -> u64 {
        self.scenes
            .prose_version_of(&id.to_string())
            .await
            .expect("the scene is there")
    }
}

#[tokio::test(start_paused = true)]
async fn an_image_added_to_a_scene_is_told_with_the_prose_version_it_was_read_at() {
    // given: a joined scene holding prose
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let writer = Writer::new();
    bench
        .writing(&present, writer.a_paragraph("The loom stood silent."))
        .await;

    // when: a figure of an image is added
    bench.writing(&present, writer.a_figure("asset_loom")).await;

    // then: the image is told, with the version of the prose it was read from
    let told = bench.assets_told(id).await;
    assert_eq!(
        told.last(),
        Some(&SceneAssetsChangedDTO {
            scene: id.to_string(),
            assets: vec!["asset_loom".to_owned()],
            version: bench.prose_version(id).await,
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_retelling_holding_the_same_images_tells_nothing_more() {
    // given: a joined scene whose image was told
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let writer = Writer::new();
    bench.writing(&present, writer.a_figure("asset_loom")).await;
    let before = bench.assets_told(id).await;

    // when: prose is added, and the scene is counted again
    bench
        .writing(&present, writer.a_paragraph("The loom stood silent."))
        .await;

    // then: the images are not told again
    assert_eq!(
        bench.assets_told(id).await,
        before,
        "the scene still holds the same image, so there is nothing new to tell"
    );
    // and: they were told once
    assert_eq!(before.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_scene_whose_figure_is_removed_is_told_to_hold_no_images() {
    // given: a joined scene holding an image
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let writer = Writer::new();
    bench.writing(&present, writer.a_figure("asset_loom")).await;

    // when: the figure is removed
    bench.writing(&present, writer.without_figures()).await;

    // then: the scene is told to hold no images
    let told = bench.assets_told(id).await;
    assert_eq!(
        told.last(),
        Some(&SceneAssetsChangedDTO {
            scene: id.to_string(),
            assets: vec![],
            version: bench.prose_version(id).await,
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_scene_counted_for_the_first_time_tells_that_it_holds_no_images() {
    // given: a scene with prose stored but never counted
    let bench = a_workbench();
    let id = bench.a_scene().await;
    bench
        .scenes
        .apply(
            &id.to_string(),
            &Writer::new().a_paragraph("The loom stood silent."),
        )
        .await
        .expect("the prose is stored");

    // when: the untold are told
    bench
        .live
        .tell_the_untold(10)
        .await
        .expect("counting should succeed");

    // then: its empty set of images is told
    assert_eq!(
        bench.assets_told(id).await,
        vec![SceneAssetsChangedDTO {
            scene: id.to_string(),
            assets: vec![],
            version: bench.prose_version(id).await,
        }],
        "nothing told yet counts as different, so a scene that lost its images while nobody counted is set right"
    );
}
