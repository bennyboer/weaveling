use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use access::{AccessCatalog, AccessError, InMemoryAccess, KeptAccess, Role};
use async_trait::async_trait;
use axum::Router;
use chores::Chore;
use clock::Clock;
use eventsourcing::{AccountId, InMemoryEventStore, Version};
use inbox::InMemoryInbox;
use live::Topic;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher, Listener};
use outbox::{InMemoryOutbox, Outbox};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::ProjectsCore;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};
use trash::InMemoryTrash;
use wiring::Context;

const CATALOGUING: &str = "catalogue-project";
const PURGING: &str = "purge-the-projects-trash";
const ABANDONING: &str = "delete-abandoned-project";

pub struct Wired {
    pub projects: ProjectsCore,
    pub routes: Router,
    pub topics: Vec<Arc<dyn Topic>>,
    pub outbox: Arc<InMemoryOutbox>,
    pub consuming: DeliveryConsumer,
    pub catalog: Arc<InMemoryProjectCatalog>,
    pub trash: Arc<InMemoryTrash>,
    pub projector: Arc<dyn Listener>,
    pub abandoning: Arc<dyn Listener>,
    pub purging: Arc<dyn Chore>,
    pub access: Arc<Unreliable>,
    versions: AtomicU64,
}

/// Who may do what, kept in memory, which can be made unreachable.
#[derive(Default)]
pub struct Unreliable {
    kept: InMemoryAccess,
    down: AtomicBool,
}

impl Unreliable {
    pub fn go_down(&self) {
        self.down.store(true, Ordering::SeqCst);
    }

    fn reachable(&self) -> Result<(), AccessError> {
        if self.down.load(Ordering::SeqCst) {
            Err(AccessError::Backend("the list is down".into()))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl AccessCatalog for Unreliable {
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

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    wired_with(clock, false)
}

/// Wired with accounts: who may do what is what the feature heard of the members, or was given by `give`.
pub fn wired_with_accounts(clock: Arc<dyn Clock>) -> Wired {
    wired_with(clock, true)
}

fn wired_with(clock: Arc<dyn Clock>, with_accounts: bool) -> Wired {
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        outbox.clone(),
        projects_messaging::message_for,
    ));
    let catalog = Arc::new(InMemoryProjectCatalog::new());
    let trash = Arc::new(InMemoryTrash::new());
    let access = Arc::new(Unreliable::default());
    let ports = projects_wiring::Ports {
        events: store.clone(),
        transactions: Arc::new(InMemoryTransactions::new()),
        catalog: catalog.clone(),
        trash: trash.clone(),
        access: KeptAccess::new(access.clone()),
        inbox: Arc::new(InMemoryInbox::new(clock.clone())),
        outbox: outbox.clone(),
    };
    let context = Context {
        clock,
        publisher: dispatcher.clone(),
        with_accounts,
    };
    let wired = projects_wiring::wire(&ports, &context);

    for listener in &wired.listeners {
        dispatcher.listen(listener.clone());
    }

    let projector = wired
        .listeners
        .iter()
        .find(|listener| listener.named().as_str() == CATALOGUING)
        .cloned()
        .expect("the feature should wire a catalog projector");
    let abandoning = wired
        .listeners
        .iter()
        .find(|listener| listener.named().as_str() == ABANDONING)
        .cloned()
        .expect("the feature should wire a listener deleting a project nobody is left in");
    let purging = wired
        .chores
        .iter()
        .find(|chore| chore.named() == PURGING)
        .cloned()
        .expect("the feature should wire a chore that purges its trash");

    Wired {
        projects: projects_wiring::core(&ports, &context),
        routes: wired
            .routes
            .merge(live::live_socket(live::Topics::of(wired.topics.clone()))),
        topics: wired.topics,
        outbox,
        consuming: DeliveryConsumer::new(dispatcher.clone(), deliveries, context.clock.clone()),
        catalog,
        trash,
        projector,
        abandoning,
        purging,
        access,
        versions: AtomicU64::new(0),
    }
}

impl Wired {
    /// Gives an account a role in a project, or takes it away with none, as the members would tell it.
    pub async fn give(&self, project: &str, account: &str, role: Option<Role>) {
        let version = self.versions.fetch_add(1, Ordering::SeqCst) + 1;
        self.projects
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

    pub async fn purged(&self) -> u64 {
        let purged = self
            .purging
            .carry_out()
            .await
            .expect("purging should succeed");
        self.settle().await;

        purged
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
