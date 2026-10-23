use std::sync::Arc;

use accounts_catalog::InMemoryAccountCatalog;
use accounts_core::{AccountCatalog, AccountEvent, AccountService, SecretStores};
use accounts_messaging::CatalogueAccountsListener;
use accounts_secrets::InMemorySecrets;
use axum::Router;
use eventsourcing::EventStore;
use mailing::{LoggingMailer, Mailer};
use messaging::Listener;
use outbox::Outbox;
use wiring::{Context, EventSourced, Feature, Storage, Unprepared, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<AccountEvent>>,
    pub secrets: SecretStores,
    pub catalog: Arc<dyn AccountCatalog>,
    pub mailer: Arc<dyn Mailer>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn service(ports: &Ports, context: &Context) -> AccountService {
    AccountService::new(
        ports.events.clone(),
        ports.secrets.clone(),
        ports.mailer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    Wired::serving(accounts_rest::router(service(ports, context)))
        .listening(listeners(ports, context))
}

fn listeners(ports: &Ports, context: &Context) -> Vec<Arc<dyn Listener>> {
    vec![Arc::new(CatalogueAccountsListener::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    ))]
}

pub struct AssembledAccounts {
    pub open_routes: Router,
    pub guarded_routes: Router,
    pub service: AccountService,
    pub outbox: Arc<dyn Outbox>,
    pub listeners: Vec<Arc<dyn Listener>>,
}

pub async fn assembled_accounts(
    storage: &Storage,
    context: &Context,
    mailer: Arc<dyn Mailer>,
) -> Result<AssembledAccounts, Unprepared> {
    let ports = Ports {
        mailer,
        ..wiring::ports::<AccountFeature>(storage, context).await?
    };
    let service = service(&ports, context);

    Ok(AssembledAccounts {
        open_routes: accounts_rest::router(service.clone()),
        guarded_routes: accounts_rest::guarded_router(service.clone()),
        service,
        outbox: ports.outbox.clone(),
        listeners: listeners(&ports, context),
    })
}

pub struct AccountFeature;

impl Feature for AccountFeature {
    const NAME: &'static str = "accounts";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(accounts_messaging::message_for, context);
        let secrets = Arc::new(InMemorySecrets::default());

        Ports {
            events,
            secrets: SecretStores::kept_by(secrets),
            catalog: Arc::new(InMemoryAccountCatalog::new()),
            mailer: Arc::new(LoggingMailer),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            accounts_store::codec(),
            accounts_messaging::message_for,
            context,
        );
        let secrets = Arc::new(accounts_secrets::PostgresSecrets::new(pool.clone()));

        Ok(Ports {
            events,
            secrets: SecretStores::kept_by(secrets),
            catalog: Arc::new(accounts_catalog::PostgresAccountCatalog::new(pool)),
            mailer: Arc::new(LoggingMailer),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            accounts_secrets::postgres::migrations(),
            accounts_catalog::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            accounts_store::codec(),
            accounts_messaging::message_for,
            context,
        );
        let secrets = Arc::new(accounts_secrets::SqliteSecrets::new(pool.clone()));

        Ok(Ports {
            events,
            secrets: SecretStores::kept_by(secrets),
            catalog: Arc::new(accounts_catalog::SqliteAccountCatalog::new(pool)),
            mailer: Arc::new(LoggingMailer),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            accounts_secrets::sqlite::migrations(),
            accounts_catalog::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
