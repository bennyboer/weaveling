use std::sync::Arc;

use accounts_core::{
    AccountEvent, AccountService, EmailAddresses, Passwords, Sessions, Verifications,
};
use accounts_secrets::InMemorySecrets;
use eventsourcing::{EventStore, Recorded};
use mailing::{LoggingMailer, Mailer};
use messaging::Message;
use outbox::Outbox;
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<AccountEvent>>,
    pub email_addresses: Arc<dyn EmailAddresses>,
    pub passwords: Arc<dyn Passwords>,
    pub verifications: Arc<dyn Verifications>,
    pub sessions: Arc<dyn Sessions>,
    pub mailer: Arc<dyn Mailer>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn service(ports: &Ports, context: &Context) -> AccountService {
    AccountService::new(
        ports.events.clone(),
        ports.email_addresses.clone(),
        ports.passwords.clone(),
        ports.verifications.clone(),
        ports.sessions.clone(),
        ports.mailer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    Wired::serving(accounts_rest::router(service(ports, context)))
}

pub fn nothing_told(_happened: &Recorded<AccountEvent>) -> Option<Message> {
    None
}

pub struct AccountFeature;

impl Feature for AccountFeature {
    const NAME: &'static str = "accounts";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } = EventSourced::in_memory(nothing_told, context);
        let secrets = Arc::new(InMemorySecrets::default());

        Ports {
            events,
            email_addresses: secrets.clone(),
            passwords: secrets.clone(),
            verifications: secrets.clone(),
            sessions: secrets,
            mailer: Arc::new(LoggingMailer),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } =
            EventSourced::on_postgres(&pool, accounts_store::codec(), nothing_told, context);
        let secrets = Arc::new(accounts_secrets::PostgresSecrets::new(pool));

        Ok(Ports {
            events,
            email_addresses: secrets.clone(),
            passwords: secrets.clone(),
            verifications: secrets.clone(),
            sessions: secrets,
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
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } =
            EventSourced::on_sqlite(&pool, accounts_store::codec(), nothing_told, context);
        let secrets = Arc::new(accounts_secrets::SqliteSecrets::new(pool));

        Ok(Ports {
            events,
            email_addresses: secrets.clone(),
            passwords: secrets.clone(),
            verifications: secrets.clone(),
            sessions: secrets,
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
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
