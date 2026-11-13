mod purging;

use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::{AllowingEverything, Authorizer};
use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use projects_catalog::InMemoryProjectCatalog;
use projects_contract::EVERY_PROJECT;
use projects_core::{ProjectCatalog, ProjectEvent, ProjectsCore};
use projects_messaging::{
    CatalogueProjectsListener, DeleteAbandonedProjectListener, RememberRolesListener,
    RememberTrashListener,
};
use transacting::Transactions;
use trash::{InMemoryTrash, PurgeTheTrash, Trash};
use wiring::{Context, EventSourced, Feature, Wired};

use crate::purging::ProjectsPurge;

pub const LIVE: &str = "broadcast-project-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<ProjectEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn ProjectCatalog>,
    pub trash: Arc<dyn Trash>,
    pub access: KeptAccess,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> ProjectsCore {
    let authorizer: Arc<dyn Authorizer> = if context.with_accounts {
        Arc::new(ports.access.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    ProjectsCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.catalog.clone(),
        ports.trash.clone(),
        ports.access.clone(),
        authorizer,
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let projects = core(ports, context);

    let broadcaster = Broadcaster::new();
    let broadcast =
        BroadcastChangesListener::new(LIVE, EVERY_PROJECT, broadcaster.clone(), |published| {
            vec![
                projects_rest::ProjectChanges::Listed,
                projects_rest::ProjectChanges::Of(published.aggregate.id.clone()),
            ]
        });

    Wired::serving(projects_rest::router(projects.clone()))
        .followed_live(vec![Arc::new(projects_rest::ProjectsTopic::new(
            projects.clone(),
            broadcaster,
        ))])
        .listening(vec![
            Arc::new(broadcast),
            Arc::new(hearing.inboxed(CatalogueProjectsListener::new(projects.clone()))),
            Arc::new(hearing.inboxed(DeleteAbandonedProjectListener::new(projects.clone()))),
            Arc::new(hearing.inboxed(RememberRolesListener::new(projects.clone()))),
            Arc::new(hearing.inboxed(RememberTrashListener::new(projects.clone()))),
        ])
        .doing_chores(vec![
            Arc::new(PurgeTheTrash::new(
                "purge-the-projects-trash",
                ports.trash.clone(),
                Arc::new(ProjectsPurge::of(projects.clone())),
                context.clock.clone(),
            )),
            Arc::new(ForgetOldTombstones::new(
                "forget-old-project-tombstones",
                chores::tombstones_by(move |before, at_most| {
                    let projects = projects.clone();
                    async move {
                        projects
                            .forget_tombstones_laid_before(before, at_most)
                            .await
                    }
                }),
                context.clock.clone(),
            )),
            Arc::new(hearing.forgetting("forget-old-project-hearings", context.clock.clone())),
        ])
}

pub struct ProjectFeature;

impl Feature for ProjectFeature {
    const NAME: &'static str = "projects";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(projects_messaging::message_for, context);

        Ports {
            events,
            transactions,
            catalog: Arc::new(InMemoryProjectCatalog::new()),
            trash: Arc::new(InMemoryTrash::new()),
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
            projects_store::codec(),
            projects_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(projects_catalog::PostgresProjectCatalog::new(pool.clone())),
            trash: Arc::new(trash::PostgresTrash::new(pool.clone())),
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
            projects_catalog::postgres::migrations(),
            inbox::postgres::migrations(),
            trash::postgres::migrations(),
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
            projects_store::codec(),
            projects_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(projects_catalog::SqliteProjectCatalog::new(pool.clone())),
            trash: Arc::new(trash::SqliteTrash::new(pool.clone())),
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
            projects_catalog::sqlite::migrations(),
            inbox::sqlite::migrations(),
            trash::sqlite::migrations(),
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
