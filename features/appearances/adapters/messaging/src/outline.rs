use appearances_core::{AppearancesCore, Place, SectionLink};

use crate::projects::project_of;
use messaging::{Failure, Handler, Message};
use outline_contract::{ATTACHED, AttachmentDTO, DETACHED, OutlineEventDTO, SECTION_REMOVED};

pub struct ProjectOutlineAppearancesListener {
    appearances: AppearancesCore,
}

impl ProjectOutlineAppearancesListener {
    pub fn new(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }
}

fn a_section(id: String) -> Place {
    Place::Section(SectionLink::from(id))
}

#[async_trait::async_trait]
impl Handler for ProjectOutlineAppearancesListener {
    const NAME: &'static str = "index-outline-appearances";
    const LISTENS_TO: &'static [&'static str] = &[ATTACHED, DETACHED, SECTION_REMOVED];
    const WHEN_REFUSED: &'static str = "Where an idea appears in the outline was not updated.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let published = eventpublishing::published_in::<OutlineEventDTO>(message)?;
        let version = published.aggregate.version;

        match published.event.body {
            OutlineEventDTO::Attached {
                attachment: AttachmentDTO::Idea { id },
                to,
                ..
            } => {
                self.appearances
                    .appeared(&id, &a_section(to), version, project_of(message).as_deref())
                    .await?
            }
            OutlineEventDTO::Detached {
                attachment: AttachmentDTO::Idea { id },
                from,
            } => {
                self.appearances
                    .disappeared(&id, &a_section(from), version)
                    .await?
            }
            OutlineEventDTO::SectionRemoved { section } => {
                self.appearances
                    .place_gone(&a_section(section), message.occurred_at)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
