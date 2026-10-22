mod accounts;
mod backend;
mod flaky;
mod mail;
mod refusals;
mod relays;

use std::sync::Arc;

use accounts_contract::ServiceModeDTO;
use accounts_core::{AccountServiceError, OperatorCode};
use accounts_wiring::{AssembledAccounts, assembled_accounts};
use appearances_wiring::AppearanceFeature;
use axum::routing::get;
use axum::{Extension, Json, Router};
use boards_wiring::BoardFeature;
use clock::Clock;
use eventsourcing::Agent;
use ideas_wiring::IdeaFeature;
use live::Broadcaster;
use mailing::{LoggingMailer, Mailer};
use messaging::{Deliveries, InMemoryDeliveries, InProcessDispatcher};
use outline_wiring::OutlineFeature;
use projects_wiring::ProjectFeature;
use scenes_wiring::SceneFeature;
use statistics_wiring::StatisticsFeature;
use tower_http::trace::TraceLayer;
use wiring::{Assembled, Context, assemble};

use crate::flaky::FlakyListener;
use crate::refusals::TellingDeliveries;

pub use accounts::{Accounts, UnreadableAccounts, WEAVELING_ACCOUNTS};
pub use backend::{Backend, Unchosen, WEAVELING_DATA, WEAVELING_DATABASE_URL};
pub use flaky::{Flakiness, Unflaky, WEAVELING_FLAKY};
pub use mail::{Mailing, WEAVELING_MAIL_DROP};
pub use relays::Relays;
pub use wiring::{Settling, Storage, Unprepared};

#[cfg(any(feature = "postgres", feature = "sqlite"))]
const MESSAGING: &str = "messaging";

pub struct Adapters {
    clock: Arc<dyn Clock>,
    dispatcher: Arc<InProcessDispatcher>,
    deliveries: Arc<dyn Deliveries>,
    refusals: Broadcaster,
    features: Vec<Assembled>,
    accounts: Option<AssembledAccounts>,
    flakiness: Option<Flakiness>,
}

impl Adapters {
    pub async fn assembled(
        storage: Storage,
        accounts: Accounts,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, Unprepared> {
        let refusals = Broadcaster::new();
        let deliveries: Arc<dyn Deliveries> = Arc::new(TellingDeliveries::around(
            deliveries(&storage).await?,
            refusals.clone(),
        ));
        let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
        let context = Context {
            clock: clock.clone(),
            publisher: dispatcher.clone(),
        };

        let features = vec![
            assemble::<ProjectFeature>(&storage, &context).await?,
            assemble::<SceneFeature>(&storage, &context).await?,
            assemble::<IdeaFeature>(&storage, &context).await?,
            assemble::<BoardFeature>(&storage, &context).await?,
            assemble::<OutlineFeature>(&storage, &context).await?,
            assemble::<AppearanceFeature>(&storage, &context).await?,
            assemble::<StatisticsFeature>(&storage, &context).await?,
        ];
        let accounts = match accounts {
            Accounts::On => Some(assembled_accounts(&storage, &context, mailer).await?),
            Accounts::Off => None,
        };

        Ok(Self {
            clock,
            dispatcher,
            deliveries,
            refusals,
            features,
            accounts,
            flakiness: None,
        })
    }

    pub async fn in_memory(accounts: Accounts, clock: Arc<dyn Clock>) -> Self {
        Self::assembled(Storage::InMemory, accounts, Arc::new(LoggingMailer), clock)
            .await
            .expect("nothing kept in memory needs preparing")
    }

    pub async fn draw_operator_code(&self) -> Result<Option<OperatorCode>, AccountServiceError> {
        match &self.accounts {
            Some(accounts) => accounts.service.draw_operator_code().await,
            None => Ok(None),
        }
    }

    pub fn refusing_on_purpose(self, flakiness: Flakiness) -> Self {
        Self {
            flakiness: Some(flakiness),
            ..self
        }
    }

    pub fn consuming(&self) -> messaging::DeliveryConsumer {
        messaging::DeliveryConsumer::new(
            self.dispatcher.clone(),
            self.deliveries.clone(),
            self.clock.clone(),
        )
    }

    pub fn settling(&self) -> Vec<Arc<dyn Settling>> {
        self.features
            .iter()
            .flat_map(|feature| feature.settling.clone())
            .collect()
    }

    pub fn outboxes(&self) -> Vec<Arc<dyn outbox::Outbox>> {
        self.features
            .iter()
            .filter_map(|feature| feature.outbox.clone())
            .chain(self.accounts.iter().map(|accounts| accounts.outbox.clone()))
            .collect()
    }
}

async fn deliveries(storage: &Storage) -> Result<Arc<dyn Deliveries>, Unprepared> {
    match storage {
        Storage::InMemory => Ok(Arc::new(InMemoryDeliveries::new())),
        #[cfg(feature = "postgres")]
        Storage::Postgres(databases) => {
            let pool = databases.ready(MESSAGING).await?;
            wiring::postgres::lay_out(MESSAGING, &pool, messaging::postgres::migrations()).await?;

            Ok(Arc::new(messaging::PostgresDeliveries::new(pool)))
        }
        #[cfg(feature = "sqlite")]
        Storage::Sqlite(files) => {
            let pool = files.ready(MESSAGING).await?;
            wiring::sqlite::lay_out(MESSAGING, &pool, messaging::sqlite::migrations()).await?;

            Ok(Arc::new(messaging::SqliteDeliveries::new(pool)))
        }
    }
}

pub fn app(adapters: Adapters) -> Router {
    let keeps_accounts = adapters.accounts.is_some();
    let service = Router::new()
        .route("/health", get(health))
        .route("/mode", get(move || mode(keeps_accounts)))
        .merge(refusals::router(
            adapters.deliveries.clone(),
            adapters.dispatcher.clone(),
            adapters.clock.clone(),
            adapters.refusals.clone(),
        ));
    let open_routes = Router::new().nest("/service", service);

    let mut guarded_routes = Router::new();
    for feature in adapters.features {
        guarded_routes = guarded_routes.merge(feature.routes);

        for listener in feature.listeners {
            adapters.dispatcher.listen(match adapters.flakiness {
                Some(flakiness) => FlakyListener::wrapping(listener, flakiness),
                None => listener,
            });
        }
    }

    let api = match adapters.accounts {
        Some(accounts) => {
            let guarded_routes = accounts_rest::admitting_only_the_signed_in(
                accounts.service,
                guarded_routes.merge(accounts.guarded_routes),
            );

            open_routes
                .merge(accounts.open_routes)
                .merge(guarded_routes)
        }
        None => open_routes.merge(guarded_routes.layer(Extension(Agent::Local))),
    };

    Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}

async fn mode(accounts: bool) -> Json<ServiceModeDTO> {
    Json(ServiceModeDTO { accounts })
}

#[cfg(test)]
mod flaky_tests;
#[cfg(test)]
mod refusals_tests;
