use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use access::{AccessCatalog, InMemoryAccess, KeptAccess, Role};
use axum::Router;
use belonging::InMemoryBelongings;
use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardEvent, BoardsCore};
use clock::Clock;
use eventsourcing::{AccountId, InMemoryEventStore, Version};
use inbox::InMemoryInbox;
use live::Topic;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher, Listener};
use outbox::{InMemoryOutbox, Outbox};
use registry::InMemoryRegistry;
use transacting::InMemoryTransactions;
use wiring::Context;

const CATALOGUING: &str = "catalogue-board";
const INDEXING: &str = "index-pinned-ideas";
const UNPINNING_DISCARDED_IDEAS: &str = "unpin-discarded-idea";
const LETTING_GO_EVERYWHERE: &str = "let-go-of-discarded-idea-everywhere";
const LETTING_GO_OF_THE_GONE: &str = "let-go-of-gone-ideas";
const REMEMBERING_PROJECTS_OF_IDEAS: &str = "boards-remember-projects-of-ideas";

pub struct Wired {
    pub boards: BoardsCore,
    pub routes: Router,
    pub topics: Vec<Arc<dyn Topic>>,
    pub store: Arc<InMemoryEventStore<BoardEvent>>,
    pub outbox: Arc<InMemoryOutbox>,
    pub consuming: DeliveryConsumer,
    pub registry: Arc<InMemoryRegistry>,
    pub catalog: Arc<InMemoryBoardCatalog>,
    pub projector: Arc<dyn Listener>,
    pub indexer: Arc<dyn Listener>,
    pub on_idea_discarded: Arc<dyn Listener>,
    pub letting_go_everywhere: Arc<dyn Listener>,
    pub letting_go_of_the_gone: Arc<dyn Listener>,
    pub remembering_projects_of_ideas: Arc<dyn Listener>,
    pub clock: Arc<dyn Clock>,
    told: AtomicU64,
}

/// Boards without accounts, where everyone may do everything.
pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    wired_keeping(clock, Arc::new(InMemoryAccess::new()), false)
}

/// Boards with accounts, who may do what kept in their own access.
pub fn wired_with_accounts(clock: Arc<dyn Clock>) -> Wired {
    wired_keeping(clock, Arc::new(InMemoryAccess::new()), true)
}

pub fn wired_keeping(
    clock: Arc<dyn Clock>,
    access: Arc<dyn AccessCatalog>,
    with_accounts: bool,
) -> Wired {
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        outbox.clone(),
        boards_messaging::message_for,
    ));
    let registry = Arc::new(InMemoryRegistry::new());
    let catalog = Arc::new(InMemoryBoardCatalog::new());
    let ports = boards_wiring::Ports {
        events: store.clone(),
        transactions: Arc::new(InMemoryTransactions::new()),
        catalog: catalog.clone(),
        registry: registry.clone(),
        belongings: Arc::new(InMemoryBelongings::new()),
        access: KeptAccess::new(access),
        inbox: Arc::new(InMemoryInbox::new(clock.clone())),
        outbox: outbox.clone(),
    };
    let context = Context {
        with_accounts,
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = boards_wiring::wire(&ports, &context);

    for listener in &wired.listeners {
        dispatcher.listen(listener.clone());
    }

    let named = |wanted: &str| {
        wired
            .listeners
            .iter()
            .find(|listener| listener.named().as_str() == wanted)
            .cloned()
            .unwrap_or_else(|| panic!("the feature should wire a {wanted} listener"))
    };
    let projector = named(CATALOGUING);
    let indexer = named(INDEXING);
    let on_idea_discarded = named(UNPINNING_DISCARDED_IDEAS);
    let letting_go_everywhere = named(LETTING_GO_EVERYWHERE);
    let letting_go_of_the_gone = named(LETTING_GO_OF_THE_GONE);
    let remembering_projects_of_ideas = named(REMEMBERING_PROJECTS_OF_IDEAS);

    Wired {
        boards: boards_wiring::core(&ports, &context),
        routes: wired
            .routes
            .merge(live::live_socket(live::Topics::of(wired.topics.clone()))),
        topics: wired.topics,
        store,
        outbox,
        consuming: DeliveryConsumer::new(dispatcher.clone(), deliveries, context.clock.clone()),
        registry,
        catalog,
        projector,
        indexer,
        on_idea_discarded,
        letting_go_everywhere,
        letting_go_of_the_gone,
        remembering_projects_of_ideas,
        clock: context.clock.clone(),
        told: AtomicU64::new(0),
    }
}

impl Wired {
    /// Tells the boards an account's role in a project, or that it is a member no more, each told newer than the last.
    pub async fn give(&self, project: &str, account: &AccountId, role: Option<Role>) {
        self.boards
            .remember_role(
                None,
                project,
                account,
                Version::of(self.told.fetch_add(1, Ordering::AcqRel) + 1),
                role,
            )
            .await
            .expect("remembering a role should succeed");
    }

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
