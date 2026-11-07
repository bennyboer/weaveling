use std::sync::Arc;

use accounts_catalog::InMemoryAccountCatalog;
use accounts_core::{
    AccountCatalog, AccountEvent, AccountsCore, KeyCeremonies, SecretStores, SigningUp, TotpKey,
};
use accounts_messaging::{
    CatalogueAccountsListener, KeepInvitationTicketsListener, RememberTrashedProjectsListener,
};
use accounts_secrets::InMemorySecrets;
use axum::Router;
use chores::Chore;
use eventsourcing::EventStore;
use inbox::{Hearing, InMemoryInbox, Inbox};
use mailing::{LoggingMailer, Mailer};
use messaging::Listener;
use outbox::Outbox;
use throttling::InMemoryThrottles;
use transacting::Transactions;
use wiring::{Context, EventSourced, Feature, Storage, Unprepared, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<AccountEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub secrets: SecretStores,
    pub catalog: Arc<dyn AccountCatalog>,
    pub inbox: Arc<dyn Inbox>,
    pub mailer: Arc<dyn Mailer>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> AccountsCore {
    AccountsCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.secrets.clone(),
        ports.catalog.clone(),
        ports.mailer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    Wired::serving(accounts_rest::router(core(ports, context)))
        .listening(listeners(ports, context))
        .doing_chores(chores(ports, context))
}

fn listeners(ports: &Ports, context: &Context) -> Vec<Arc<dyn Listener>> {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    vec![
        Arc::new(hearing.inboxed(CatalogueAccountsListener::new(core(ports, context)))),
        Arc::new(hearing.inboxed(KeepInvitationTicketsListener::new(core(ports, context)))),
        Arc::new(hearing.inboxed(RememberTrashedProjectsListener::new(core(ports, context)))),
    ]
}

fn chores(ports: &Ports, context: &Context) -> Vec<Arc<dyn Chore>> {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    vec![Arc::new(hearing.forgetting(
        "forget-old-account-hearings",
        context.clock.clone(),
    ))]
}

pub struct AssembledAccounts {
    pub open_routes: Router,
    pub guarded_routes: Router,
    pub core: AccountsCore,
    pub outbox: Arc<dyn Outbox>,
    pub listeners: Vec<Arc<dyn Listener>>,
    pub chores: Vec<Arc<dyn Chore>>,
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

    Ok(AssembledAccounts::serving(
        core(&ports, context),
        ports.outbox.clone(),
        listeners(&ports, context),
        chores(&ports, context),
    ))
}

impl AssembledAccounts {
    fn serving(
        core: AccountsCore,
        outbox: Arc<dyn Outbox>,
        listeners: Vec<Arc<dyn Listener>>,
        chores: Vec<Arc<dyn Chore>>,
    ) -> Self {
        Self {
            open_routes: accounts_rest::router(core.clone()),
            guarded_routes: accounts_rest::guarded_router(core.clone()),
            core,
            outbox,
            listeners,
            chores,
        }
    }

    pub fn signing_up(self, signing_up: SigningUp) -> Self {
        Self::serving(
            self.core.signing_up(signing_up),
            self.outbox,
            self.listeners,
            self.chores,
        )
    }

    pub fn sealing_second_factors(self, key: TotpKey) -> Self {
        Self::serving(
            self.core.sealing_second_factors(key),
            self.outbox,
            self.listeners,
            self.chores,
        )
    }

    pub fn holding_key_ceremonies(self, ceremonies: KeyCeremonies) -> Self {
        Self::serving(
            self.core.holding_key_ceremonies(ceremonies),
            self.outbox,
            self.listeners,
            self.chores,
        )
    }
}

pub struct AccountFeature;

impl Feature for AccountFeature {
    const NAME: &'static str = "accounts";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(accounts_messaging::message_for, context);
        let secrets = Arc::new(InMemorySecrets::default());

        Ports {
            events,
            transactions,
            secrets: SecretStores::kept_by(secrets, Arc::new(InMemoryThrottles::default())),
            catalog: Arc::new(InMemoryAccountCatalog::new()),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
            mailer: Arc::new(LoggingMailer),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_postgres(
            &pool,
            accounts_store::codec(),
            accounts_messaging::message_for,
            context,
        );
        let secrets = Arc::new(accounts_secrets::PostgresSecrets::new(pool.clone()));
        let throttles = Arc::new(throttling::PostgresThrottles::new(pool.clone()));

        Ok(Ports {
            events,
            transactions,
            secrets: SecretStores::kept_by(secrets, throttles),
            catalog: Arc::new(accounts_catalog::PostgresAccountCatalog::new(pool)),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
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
            throttling::postgres::migrations(),
            inbox::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_sqlite(
            &pool,
            accounts_store::codec(),
            accounts_messaging::message_for,
            context,
        );
        let secrets = Arc::new(accounts_secrets::SqliteSecrets::new(pool.clone()));
        let throttles = Arc::new(throttling::SqliteThrottles::new(pool.clone()));

        Ok(Ports {
            events,
            transactions,
            secrets: SecretStores::kept_by(secrets, throttles),
            catalog: Arc::new(accounts_catalog::SqliteAccountCatalog::new(pool)),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
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
            throttling::sqlite::migrations(),
            inbox::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
