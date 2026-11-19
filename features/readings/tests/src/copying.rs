use std::sync::Arc;

use access::KeptAccess;
use authorizing::Authorizer;
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher};
use outbox::{InMemoryOutbox, Outbox};
use readings_catalog::InMemoryReadingCopyCatalog;
use readings_core::{NobodyFollows, ReadingsCore};
use readings_messaging::CatalogueReadingCopiesListener;
use transacting::InMemoryTransactions;

use crate::sample::SampleBook;

/// The core around the sample book, whose copies' events reach the list of copies once they are settled.
pub struct Copying {
    pub readings: ReadingsCore,
    outbox: Arc<InMemoryOutbox>,
    consuming: DeliveryConsumer,
}

pub fn a_workbench(
    access: KeptAccess,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
) -> Copying {
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let transactions = Arc::new(InMemoryTransactions::new());
    let readings = ReadingsCore::new(
        Arc::new(SampleBook),
        Arc::new(SampleBook),
        Arc::new(InMemoryReadingCopyCatalog::new()),
        Arc::new(InMemoryEventStore::enqueuing_to(
            outbox.clone(),
            readings_messaging::message_for,
        )),
        Arc::new(NobodyFollows),
        transactions.clone(),
        access,
        authorizer,
        clock.clone(),
    );
    dispatcher.listen(Arc::new(Inboxed::new(
        CatalogueReadingCopiesListener::new(readings.clone()),
        transactions,
        Arc::new(InMemoryInbox::new(clock.clone())),
    )));

    Copying {
        readings,
        outbox,
        consuming: DeliveryConsumer::new(dispatcher, deliveries, clock),
    }
}

impl Copying {
    /// Delivers every message the copies told, until none is left.
    pub async fn settle(&self) {
        loop {
            let published = self
                .outbox
                .deliver(128)
                .await
                .expect("delivering should succeed")
                .published;
            let taken = self.consuming.drain(128).await;

            if published == 0 && taken == 0 {
                break;
            }
        }
    }
}
