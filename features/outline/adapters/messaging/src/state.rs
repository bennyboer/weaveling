use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::Standing;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use outbox::{Origin, Outbox};
use outline_contract::{
    ATTACHED, DETACHED, DISCARDED, OutlineStateDTO, SECTION_ADDED, SECTION_DEMOTED, SECTION_MOVED,
    SECTION_PROMOTED, SECTION_REMOVED, SECTION_RETITLED, STATE_CHANGED, STARTED, SectionStateDTO,
};
use outline_core::{KIND, Outline, OutlineId, OutlinesCore};
use serde::de::IgnoredAny;
use transacting::Transaction;

pub struct TellStateListener {
    outlines: OutlinesCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl TellStateListener {
    pub fn new(outlines: OutlinesCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            outlines,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for TellStateListener {
    const NAME: &'static str = "tell-outline-state";
    const LISTENS_TO: &'static [&'static str] = &[
        STARTED,
        SECTION_ADDED,
        SECTION_RETITLED,
        SECTION_MOVED,
        SECTION_PROMOTED,
        SECTION_DEMOTED,
        SECTION_REMOVED,
        ATTACHED,
        DETACHED,
        DISCARDED,
    ];
    const WHEN_REFUSED: &'static str = "A change to an outline was not told to other features.";

    /// Tells the outline's whole state once the heard event is the outline's newest.
    /// A command that appends several events is told once, by its last event.
    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let outline = eventpublishing::aggregate_in::<OutlineId>(message)?;
        let heard = eventpublishing::published_in::<IgnoredAny>(message)?
            .aggregate
            .version;
        let standing = self.outlines.standing(&outline).await?;
        if standing.version.count() > heard {
            return Ok(());
        }

        self.outbox
            .enqueue_follow_up_within(
                transaction,
                Origin {
                    aggregate: &outline.to_string(),
                    kind: KIND.as_str(),
                    version: Some(standing.version.count()),
                },
                message,
                STATE_CHANGED,
                &to_state_dto(&outline, &standing),
                self.clock.now(),
            )
            .await?;

        Ok(())
    }
}

fn to_state_dto(outline: &OutlineId, standing: &Standing<Outline>) -> OutlineStateDTO {
    OutlineStateDTO {
        outline: outline.to_string(),
        project: standing.state.project().to_string(),
        version: standing.version.count(),
        discarded: standing.state.is_discarded(),
        sections: standing
            .state
            .sections()
            .into_iter()
            .map(|placed| SectionStateDTO {
                section: placed.section.to_string(),
                parent: placed.parent.map(|parent| parent.to_string()),
                title: placed.title.to_string(),
                attachments: placed
                    .attachments
                    .iter()
                    .map(crate::publishing::to_dto)
                    .collect(),
            })
            .collect(),
    }
}
