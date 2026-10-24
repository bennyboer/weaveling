use std::sync::Arc;

use appearances_core::{AppearanceCatalog, IdeaLink, Place, SectionLink, Subject};
use messaging::{Failure, Handler, Message};
use outline_contract::{ATTACHED, AttachmentDTO, DETACHED, OutlineEventDTO, SECTION_REMOVED};

pub struct ProjectOutlineAppearancesListener {
    catalog: Arc<dyn AppearanceCatalog>,
}

impl ProjectOutlineAppearancesListener {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
    }
}

fn an_idea(id: String) -> Subject {
    Subject::Idea(IdeaLink::from(id))
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
                self.catalog
                    .remember(&an_idea(id), &a_section(to), version)
                    .await?
            }
            OutlineEventDTO::Detached {
                attachment: AttachmentDTO::Idea { id },
                from,
            } => {
                self.catalog
                    .forget(&an_idea(id), &a_section(from), version)
                    .await?
            }
            OutlineEventDTO::SectionRemoved { section } => {
                self.catalog.forget_place(&a_section(section)).await?;
            }
            _ => {}
        }

        Ok(())
    }
}
