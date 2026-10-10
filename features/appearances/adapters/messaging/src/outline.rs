use std::sync::Arc;

use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Place, SectionLink, Subject};
use eventpublishing::{UnreadableMessage, published_in};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_contract::{ATTACHED, AttachmentDTO, DETACHED, OutlineEventDTO, SECTION_REMOVED};
use thiserror::Error;

const NAME: &str = "index-outline-appearances";

pub struct OutlineAppearancesProjector {
    catalog: Arc<dyn AppearanceCatalog>,
}

#[derive(Debug, Error)]
enum NotIndexed {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl OutlineAppearancesProjector {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
    }

    async fn index(&self, message: &Message) -> Result<(), NotIndexed> {
        let published = published_in::<OutlineEventDTO>(message)?;
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
                self.catalog.forget_place(&a_section(section)).await?
            }
            _ => {}
        }

        Ok(())
    }
}

fn an_idea(id: String) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

fn a_section(id: String) -> Place {
    Place::Section(SectionLink::from(id))
}

#[async_trait::async_trait]
impl Listener for OutlineAppearancesProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME)
            .expect("the outline appearances listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        [ATTACHED, DETACHED, SECTION_REMOVED]
            .into_iter()
            .map(|key| Subscription::parse(key).expect("a declared routing key holds no wildcards"))
            .collect()
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.index(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
