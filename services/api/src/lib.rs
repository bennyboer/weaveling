mod backend;
mod flaky;
mod refusals;
mod relays;

use std::sync::Arc;

use appearances_wiring::AppearanceFeature;
use axum::Router;
use axum::routing::get;
use boards_wiring::BoardFeature;
use clock::Clock;
use ideas_wiring::IdeaFeature;
use messaging::{Deliveries, InMemoryDeliveries, InProcessDispatcher};
use outline_wiring::OutlineFeature;
use projects_wiring::ProjectFeature;
use scenes_wiring::SceneFeature;
use tower_http::trace::TraceLayer;
use wiring::{Assembled, Context, assemble};

use crate::flaky::Flaky;

pub use backend::{Backend, Unchosen, WEAVELING_DATA, WEAVELING_DATABASE_URL};
pub use flaky::{Flakiness, Unflaky, WEAVELING_FLAKY};
pub use relays::Relays;
pub use wiring::{Storage, Unprepared};

#[cfg(any(feature = "postgres", feature = "sqlite"))]
const MESSAGING: &str = "messaging";

pub struct Adapters {
    clock: Arc<dyn Clock>,
    dispatcher: Arc<InProcessDispatcher>,
    deliveries: Arc<dyn Deliveries>,
    features: Vec<Assembled>,
    flakiness: Option<Flakiness>,
}

impl Adapters {
    pub async fn assembled(storage: Storage, clock: Arc<dyn Clock>) -> Result<Self, Unprepared> {
        let deliveries = deliveries(&storage).await?;
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
        ];

        Ok(Self {
            clock,
            dispatcher,
            deliveries,
            features,
            flakiness: None,
        })
    }

    pub async fn in_memory(clock: Arc<dyn Clock>) -> Self {
        Self::assembled(Storage::InMemory, clock)
            .await
            .expect("nothing kept in memory needs preparing")
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

    pub fn outboxes(&self) -> Vec<Arc<dyn outbox::Outbox>> {
        self.features
            .iter()
            .filter_map(|feature| feature.outbox.clone())
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
    let service = Router::new()
        .route("/health", get(health))
        .merge(refusals::router(
            adapters.deliveries.clone(),
            adapters.dispatcher.clone(),
            adapters.clock.clone(),
        ));
    let mut api = Router::new().nest("/service", service);

    for feature in adapters.features {
        api = api.merge(feature.routes);

        for listener in feature.listeners {
            adapters.dispatcher.listen(match adapters.flakiness {
                Some(flakiness) => Flaky::wrapping(listener, flakiness),
                None => listener,
            });
        }
    }

    Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
mod flaky_tests;
#[cfg(test)]
mod refusals_tests;
