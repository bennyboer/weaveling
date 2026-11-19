use std::sync::Arc;

use access::KeptAccess;
use authorizing::Authorizer;
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher, Message, Publisher};
use outbox::{InMemoryOutbox, Outbox};
use readings_catalog::{
    InMemoryAccountNameCatalog, InMemoryReadingCopyCatalog, InMemoryReadingCopyContents,
};
use readings_core::{NobodyFollows, ReadingsCore};
use readings_messaging::{
    CatalogueReadingCopiesListener, DeleteCopiesOfDeletedProjectListener,
    SweepCopiesOfDeletedProjectListener,
};
use transacting::InMemoryTransactions;

use crate::sample::SampleBook;

/// The core around the sample book, whose copies' events reach the list of copies once they are settled,
/// and which sweeps the copies of a project it is told was deleted.
pub struct Copying {
    pub readings: ReadingsCore,
    pub contents: Arc<InMemoryReadingCopyContents>,
    outbox: Arc<InMemoryOutbox>,
    dispatcher: Arc<InProcessDispatcher>,
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
    let contents = Arc::new(InMemoryReadingCopyContents::new());
    let readings = ReadingsCore::new(
        Arc::new(SampleBook),
        Arc::new(SampleBook),
        Arc::new(InMemoryReadingCopyCatalog::new()),
        Arc::new(InMemoryAccountNameCatalog::new()),
        contents.clone(),
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
    let inbox = Arc::new(InMemoryInbox::new(clock.clone()));
    dispatcher.listen(Arc::new(Inboxed::new(
        CatalogueReadingCopiesListener::new(readings.clone()),
        transactions.clone(),
        inbox.clone(),
    )));
    dispatcher.listen(Arc::new(Inboxed::new(
        SweepCopiesOfDeletedProjectListener::new(outbox.clone(), clock.clone()),
        transactions,
        inbox,
    )));
    dispatcher.listen(Arc::new(DeleteCopiesOfDeletedProjectListener::new(
        readings.clone(),
    )));

    Copying {
        readings,
        contents,
        outbox,
        dispatcher: dispatcher.clone(),
        consuming: DeliveryConsumer::new(dispatcher, deliveries, clock),
    }
}

impl Copying {
    /// Tells the readings a message from another feature, and settles what follows from it.
    pub async fn told(&self, message: Message) {
        self.dispatcher
            .publish(message)
            .await
            .expect("telling should succeed");
        self.settle().await;
    }

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
