use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use access::{AccessCatalog, AccessError, HearsAccess, InMemoryAccess, KeptAccess, Role};
use async_trait::async_trait;
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
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};
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

/// Who may do what, kept in memory, that can be made unreachable.
#[derive(Default)]
pub struct Faltering {
    kept: InMemoryAccess,
    pub down: AtomicBool,
}

impl Faltering {
    fn reachable(&self) -> Result<(), AccessError> {
        if self.down.load(Ordering::Acquire) {
            return Err(AccessError::Backend("the list is down".into()));
        }

        Ok(())
    }
}

#[async_trait]
impl AccessCatalog for Faltering {
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_role(transaction, project, account, version, role, at)
            .await
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_disbanded(transaction, project, version, at)
            .await
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_trash(transaction, project, version, in_the_trash)
            .await
    }

    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
            .remember_operator(transaction, account, version, operator, at)
            .await
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
        self.kept
            .forget_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError> {
        self.reachable()?;
        self.kept.role_of(project, account).await
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.is_in_the_trash(project).await
    }

    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError> {
        self.reachable()?;
        self.kept.projects_of(account).await
    }

    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.share_a_project(one, other).await
    }

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.operates(account).await
    }
}
