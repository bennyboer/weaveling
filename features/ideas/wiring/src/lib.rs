use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::{AllowingEverything, Authorizer};
use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use ideas_catalog::InMemoryIdeaCatalog;
use ideas_core::{IdeaCatalog, IdeaEvent, IdeasCore};
use ideas_messaging::{
    BroadcastIdeaChangesListener, CatalogueIdeasListener, DiscardOnProjectDeletedListener,
    RememberRolesListener, RememberTrashListener,
};
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::Broadcaster;
use outbox::Outbox;
use transacting::Transactions;
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<IdeaEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn IdeaCatalog>,
    pub access: KeptAccess,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> IdeasCore {
    let authorizer: Arc<dyn Authorizer> = if context.with_accounts {
        Arc::new(ports.access.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    IdeasCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.catalog.clone(),
        ports.access.clone(),
        authorizer,
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let ideas = core(ports, context);
    let sweep = DiscardOnProjectDeletedListener::new(
        ideas.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );

    let broadcaster = Broadcaster::new();
    let broadcast = BroadcastIdeaChangesListener::new(
        ideas.clone(),
        broadcaster.clone(),
        ideas_rest::IdeaChanges,
    );

    Wired::serving(ideas_rest::router(ideas.clone()))
        .followed_live(vec![ideas_rest::topic(ideas.clone(), broadcaster)])
        .listening(vec![
            Arc::new(broadcast),
            Arc::new(hearing.inboxed(CatalogueIdeasListener::new(ideas.clone()))),
            Arc::new(hearing.inboxed(RememberRolesListener::new(ideas.clone()))),
            Arc::new(hearing.inboxed(RememberTrashListener::new(ideas.clone()))),
            Arc::new(sweep),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-idea-tombstones",
                chores::tombstones_by({
                    let ideas = ideas.clone();
                    move |before, at_most| {
                        let ideas = ideas.clone();
                        async move { ideas.forget_tombstones_laid_before(before, at_most).await }
                    }
                }),
                context.clock.clone(),
            )),
            Arc::new(hearing.forgetting("forget-old-idea-hearings", context.clock.clone())),
        ])
}

pub struct IdeaFeature;

impl Feature for IdeaFeature {
    const NAME: &'static str = "ideas";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(ideas_messaging::message_for, context);

        Ports {
            events,
            transactions,
            catalog: Arc::new(InMemoryIdeaCatalog::new()),
            access: KeptAccess::new(Arc::new(InMemoryAccess::new())),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_postgres(
            &pool,
            ideas_store::codec(),
            ideas_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(ideas_catalog::PostgresIdeaCatalog::new(pool.clone())),
            access: KeptAccess::new(Arc::new(access::PostgresAccess::new(pool))),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            ideas_catalog::postgres::migrations(),
            inbox::postgres::migrations(),
            access::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_sqlite(
            &pool,
            ideas_store::codec(),
            ideas_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(ideas_catalog::SqliteIdeaCatalog::new(pool.clone())),
            access: KeptAccess::new(Arc::new(access::SqliteAccess::new(pool))),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            ideas_catalog::sqlite::migrations(),
            inbox::sqlite::migrations(),
            access::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
