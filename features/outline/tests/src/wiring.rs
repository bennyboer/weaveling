use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use access::{AccessCatalog, AccessError, InMemoryAccess, KeptAccess, Role};
use async_trait::async_trait;
use axum::Router;
use belonging::InMemoryBelongings;
use clock::Clock;
use eventsourcing::{AccountId, InMemoryEventStore, Version};
use inbox::InMemoryInbox;
use live::Topic;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher, Listener};
use outbox::{InMemoryOutbox, Outbox};
use outline_catalog::InMemoryOutlineCatalog;
use outline_core::{OutlineEvent, OutlinesCore};
use registry::InMemoryRegistry;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};
use wiring::Context;

const CATALOGUING: &str = "catalogue-outline";
const INDEXING: &str = "index-attachments";
const DETACHING_DISCARDED_IDEAS: &str = "detach-discarded-idea";
const DETACHING_DELETED_SCENES: &str = "detach-deleted-scene";
const DETACHING_EVERYWHERE: &str = "detach-everywhere";
const DETACHING_THE_GONE: &str = "detach-gone-attachments";
const REMEMBERING_PROJECTS: &str = "outline-remember-projects-of-attachments";
const REMEMBERING_ROLES: &str = "outline-remember-roles";
const REMEMBERING_TRASH: &str = "outline-remember-trash";

/// Who may do what as the feature heard it, unless the list is down.
#[derive(Default)]
pub struct WhoMayDoWhat {
    kept: InMemoryAccess,
    pub down: AtomicBool,
}

impl WhoMayDoWhat {
    fn reachable(&self) -> Result<(), AccessError> {
        if self.down.load(Ordering::Acquire) {
            return Err(AccessError::Backend("the list is down".into()));
        }

        Ok(())
    }
}

#[async_trait]
impl AccessCatalog for WhoMayDoWhat {
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

pub struct Wired {
    pub outlines: OutlinesCore,
    pub reopened: Box<dyn Fn() -> OutlinesCore + Send + Sync>,
    pub routes: Router,
    pub topics: Vec<Arc<dyn Topic>>,
    pub store: Arc<InMemoryEventStore<OutlineEvent>>,
    pub outbox: Arc<InMemoryOutbox>,
    pub consuming: DeliveryConsumer,
    pub registry: Arc<InMemoryRegistry>,
    pub catalog: Arc<InMemoryOutlineCatalog>,
    pub projector: Arc<dyn Listener>,
    pub indexer: Arc<dyn Listener>,
    pub on_idea_discarded: Arc<dyn Listener>,
    pub on_scene_deleted: Arc<dyn Listener>,
    pub detaching_everywhere: Arc<dyn Listener>,
    pub detaching_the_gone: Arc<dyn Listener>,
    pub remembering_projects: Arc<dyn Listener>,
    pub remembering_roles: Arc<dyn Listener>,
    pub remembering_trash: Arc<dyn Listener>,
    pub access: Arc<WhoMayDoWhat>,
    pub clock: Arc<dyn Clock>,
    told: AtomicU64,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    wired_on(clock, false)
}

/// A feature with accounts, where nobody but the system may do anything until told a role.
pub fn wired_with_accounts(clock: Arc<dyn Clock>) -> Wired {
    wired_on(clock, true)
}

/// A feature with accounts, where the author owns the first two projects and nobody else is a member.
pub async fn wired_where_the_author_owns(clock: Arc<dyn Clock>) -> Wired {
    let wired = wired_with_accounts(clock);
    for project in ["project_1", "project_2"] {
        wired.give(project, "author-7", Some(Role::Owner)).await;
    }

    wired
}

fn wired_on(clock: Arc<dyn Clock>, with_accounts: bool) -> Wired {
    let access = Arc::new(WhoMayDoWhat::default());
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        outbox.clone(),
        outline_messaging::message_for,
    ));
    let registry = Arc::new(InMemoryRegistry::new());
    let belongings = Arc::new(InMemoryBelongings::new());
    let catalog = Arc::new(InMemoryOutlineCatalog::new());
    let ports = outline_wiring::Ports {
        events: store.clone(),
        transactions: Arc::new(InMemoryTransactions::new()),
        catalog: catalog.clone(),
        registry: registry.clone(),
        belongings: belongings.clone(),
        access: KeptAccess::new(access.clone()),
        inbox: Arc::new(InMemoryInbox::new(clock.clone())),
        outbox: outbox.clone(),
    };
    let context = Context {
        with_accounts,
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = outline_wiring::wire(&ports, &context);
    let afresh = {
        let store = store.clone();
        let catalog = catalog.clone();
        let outbox = outbox.clone();
        let registry = registry.clone();
        let belongings = belongings.clone();
        let clock = context.clock.clone();
        let publisher = dispatcher.clone();
        let kept = ports.access.clone();

        move || {
            outline_wiring::core(
                &outline_wiring::Ports {
                    events: store.clone(),
                    transactions: Arc::new(InMemoryTransactions::new()),
                    catalog: catalog.clone(),
                    registry: registry.clone(),
                    belongings: belongings.clone(),
                    access: kept.clone(),
                    inbox: Arc::new(InMemoryInbox::new(clock.clone())),
                    outbox: outbox.clone(),
                },
                &Context {
                    with_accounts,
                    clock: clock.clone(),
                    publisher: publisher.clone(),
                },
            )
        }
    };

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
    let on_idea_discarded = named(DETACHING_DISCARDED_IDEAS);
    let on_scene_deleted = named(DETACHING_DELETED_SCENES);
    let detaching_everywhere = named(DETACHING_EVERYWHERE);
    let detaching_the_gone = named(DETACHING_THE_GONE);
    let remembering_projects = named(REMEMBERING_PROJECTS);
    let remembering_roles = named(REMEMBERING_ROLES);
    let remembering_trash = named(REMEMBERING_TRASH);

    Wired {
        outlines: outline_wiring::core(&ports, &context),
        reopened: Box::new(afresh),
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
        on_scene_deleted,
        detaching_everywhere,
        detaching_the_gone,
        remembering_projects,
        remembering_roles,
        remembering_trash,
        access,
        clock: context.clock.clone(),
        told: AtomicU64::new(0),
    }
}

impl Wired {
    /// Tells the outline an account's role in a project, or that it is a member no more, each word newer than the last.
    pub async fn give(&self, project: &str, account: &str, role: Option<Role>) {
        let version = self.told.fetch_add(1, Ordering::AcqRel) + 1;
        self.outlines
            .remember_role(
                None,
                project,
                &AccountId::from(account),
                Version::of(version),
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
