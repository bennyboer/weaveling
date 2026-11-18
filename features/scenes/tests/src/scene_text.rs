use std::collections::HashMap;
use std::sync::Arc;

use outbox::Outbox;
use scenes_contract::{BlockDTO, MarksDTO, RunDTO, SceneTextStateDTO, TEXT_STATE_CHANGED};
use scenes_core::SceneId;
use yrs::types::xml::XmlOut;
use yrs::{Any, Text, XmlFragment};

use crate::scene_images::{Wired, Writer, a_workbench};

impl Wired {
    async fn text_told(&self, id: SceneId) -> Vec<SceneTextStateDTO> {
        self.outbox
            .deliver(100)
            .await
            .expect("delivering should succeed");

        self.published
            .heard()
            .into_iter()
            .filter(|message| message.routing.to_string() == TEXT_STATE_CHANGED)
            .map(|message| {
                message
                    .payload_as::<SceneTextStateDTO>()
                    .expect("the text reads back as the contract says")
            })
            .filter(|told| told.scene == id.to_string())
            .collect()
    }
}

impl Writer {
    /// Marks the first paragraph's text as speech, a mark only the writing view shows.
    fn speaking(&self) -> Vec<u8> {
        self.edited(|txn, fragment| {
            let Some(XmlOut::Element(paragraph)) = fragment.get(txn, 0) else {
                panic!("expected a paragraph");
            };
            let Some(XmlOut::Text(prose)) = paragraph.get(txn, 0) else {
                panic!("expected prose");
            };
            let length = prose.len(txn);
            prose.format(
                txn,
                0,
                length,
                HashMap::from([(Arc::from("speech"), Any::Map(Default::default()))]),
            );
        })
    }
}

fn plain(text: &str) -> RunDTO {
    RunDTO::Text {
        text: text.to_owned(),
        marks: MarksDTO::default(),
    }
}

#[tokio::test(start_paused = true)]
async fn a_scene_s_text_is_told_as_blocks_once_it_has_settled() {
    // given: a joined scene
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let writer = Writer::new();

    // when: a paragraph is written and the text settles
    bench
        .writing(&present, writer.a_paragraph("The loom stood silent."))
        .await;

    // then: the text is told as its blocks, with the version of the prose it was read from
    assert_eq!(
        bench.text_told(id).await.last(),
        Some(&SceneTextStateDTO {
            scene: id.to_string(),
            blocks: vec![BlockDTO::Paragraph {
                runs: vec![plain("The loom stood silent.")],
            }],
            version: bench.prose_version(id).await,
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_change_a_reader_does_not_see_tells_the_text_no_more() {
    // given: a joined scene whose text was told
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let writer = Writer::new();
    bench
        .writing(&present, writer.a_paragraph("What am I to do?"))
        .await;
    let before = bench.text_told(id).await;

    // when: its text is marked as speech, and the scene is counted again
    bench.writing(&present, writer.speaking()).await;

    // then: the text is not told again
    assert_eq!(
        bench.text_told(id).await,
        before,
        "speech is for the writing view, so a reader's text is as it was"
    );
    // and: it was told once
    assert_eq!(before.len(), 1);
}
