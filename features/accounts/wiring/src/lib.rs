use std::sync::Arc;

use accounts_core::{AccountEvent, AccountService, Credentials, Verifications};
use accounts_secrets::InMemorySecrets;
use eventsourcing::{EventStore, Recorded};
use mailing::{LoggingMailer, Mailer};
use messaging::Message;
use outbox::Outbox;
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<AccountEvent>>,
    pub credentials: Arc<dyn Credentials>,
    pub verifications: Arc<dyn Verifications>,
    pub mailer: Arc<dyn Mailer>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn service(ports: &Ports, context: &Context) -> AccountService {
    AccountService::new(
        ports.events.clone(),
        ports.credentials.clone(),
        ports.verifications.clone(),
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
            credentials: secrets.clone(),
            verifications: secrets,
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
            credentials: secrets.clone(),
            verifications: secrets,
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

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
