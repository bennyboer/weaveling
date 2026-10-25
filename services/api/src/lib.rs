mod accounts;
mod backend;
mod flaky;
mod mail;
mod refusals;
mod relays;
mod sign_up;

use std::sync::Arc;

use accounts_contract::ServiceModeDTO;
use accounts_core::{AccountService, AccountServiceError, OperatorCode};
use accounts_wiring::AssembledAccounts;
use appearances_wiring::AppearanceFeature;
use authorizing::{AllowingEverything, Authorizer};
use axum::routing;
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
use serving::Problem;
use statistics_wiring::StatisticsFeature;
use tower_http::trace::TraceLayer;
use wiring::{Assembled, Context};

use crate::flaky::FlakyListener;
use crate::refusals::TellingDeliveries;

pub use accounts::{Accounts, UnreadableAccounts, WEAVELING_ACCOUNTS};
pub use backend::{Backend, Unchosen, WEAVELING_DATA, WEAVELING_DATABASE_URL};
pub use chores::{Chore, Chores};
pub use flaky::{Flakiness, Unflaky, WEAVELING_FLAKY};
pub use mail::{Mailing, WEAVELING_MAIL_DROP};
pub use relays::Relays;
pub use sign_up::{SignUp, UnreadableSignUp, WEAVELING_SIGN_UP};
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
    authorizer: Arc<dyn Authorizer>,
    flakiness: Option<Flakiness>,
    sign_up: SignUp,
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
        let members = match accounts {
            Accounts::On => Some(
                members_wiring::assembled_members(&storage, clock.clone(), dispatcher.clone())
                    .await?,
            ),
            Accounts::Off => None,
        };
        let authorizer: Arc<dyn Authorizer> = match &members {
            Some(members) => members.authorizer.clone(),
            None => Arc::new(AllowingEverything),
        };
        let context = Context {
            clock: clock.clone(),
            publisher: dispatcher.clone(),
            authorizer: authorizer.clone(),
        };

        let mut features = vec![
            wiring::assemble::<ProjectFeature>(&storage, &context).await?,
            wiring::assemble::<SceneFeature>(&storage, &context).await?,
            wiring::assemble::<IdeaFeature>(&storage, &context).await?,
            wiring::assemble::<BoardFeature>(&storage, &context).await?,
            wiring::assemble::<OutlineFeature>(&storage, &context).await?,
            wiring::assemble::<AppearanceFeature>(&storage, &context).await?,
            wiring::assemble::<StatisticsFeature>(&storage, &context).await?,
        ];
        if let Some(members) = members {
            features.push(members.assembled);
        }
        let accounts = match accounts {
            Accounts::On => {
                features.push(
                    invitations_wiring::assembled_invitations(&storage, &context, mailer.clone())
                        .await?,
                );
                Some(accounts_wiring::assembled_accounts(&storage, &context, mailer).await?)
            }
            Accounts::Off => None,
        };

        Ok(Self {
            clock,
            dispatcher,
            deliveries,
            refusals,
            features,
            accounts,
            authorizer,
            flakiness: None,
            sign_up: SignUp::Open,
        })
    }

    pub async fn in_memory(accounts: Accounts, clock: Arc<dyn Clock>) -> Self {
        Self::assembled(Storage::InMemory, accounts, Arc::new(LoggingMailer), clock)
            .await
            .expect("nothing kept in memory needs preparing")
    }

    pub fn authorizer(&self) -> Arc<dyn Authorizer> {
        self.authorizer.clone()
    }

    pub async fn draw_operator_code(&self) -> Result<Option<OperatorCode>, AccountServiceError> {
        match &self.accounts {
            Some(accounts) => accounts.service.draw_operator_code().await,
            None => Ok(None),
        }
    }

    pub fn signing_up(self, sign_up: SignUp) -> Self {
        Self {
            sign_up,
            accounts: self
                .accounts
                .map(|accounts| accounts.signing_up(sign_up.into())),
            ..self
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

    pub fn chores(&self) -> Vec<Arc<dyn Chore>> {
        self.features
            .iter()
            .flat_map(|feature| feature.chores.clone())
            .collect()
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
    let told = ServiceModeDTO {
        operator_wanted: false,
        accounts: adapters.accounts.is_some(),
        sign_up: adapters
            .accounts
            .is_some()
            .then(|| adapters.sign_up.told().to_owned()),
    };
    let operating = adapters
        .accounts
        .as_ref()
        .map(|accounts| accounts.service.clone());
    let service = Router::new()
        .route("/health", routing::get(health))
        .route(
            "/mode",
            routing::get(move || mode(told.clone(), operating.clone())),
        )
        .merge(refusals::router(
            adapters.deliveries.clone(),
            adapters.dispatcher.clone(),
            adapters.clock.clone(),
            adapters.refusals.clone(),
        ));
    let open_routes = Router::new().nest("/service", service);

    let accounts_listeners = adapters
        .accounts
        .iter()
        .flat_map(|accounts| accounts.listeners.clone());
    let mut guarded_routes = Router::new();
    let mut listeners = Vec::new();
    for feature in adapters.features {
        guarded_routes = guarded_routes.merge(feature.routes);
        listeners.extend(feature.listeners);
    }
    for listener in listeners.into_iter().chain(accounts_listeners) {
        adapters.dispatcher.listen(match adapters.flakiness {
            Some(flakiness) => FlakyListener::wrapping(listener, flakiness),
            None => listener,
        });
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

async fn mode(
    told: ServiceModeDTO,
    operating: Option<AccountService>,
) -> Result<Json<ServiceModeDTO>, Problem> {
    let operator_wanted = match operating {
        Some(accounts) => accounts
            .operator_wanted()
            .await
            .map_err(Problem::unexpected)?,
        None => false,
    };

    Ok(Json(ServiceModeDTO {
        operator_wanted,
        ..told
    }))
}

#[cfg(test)]
mod flaky_tests;
#[cfg(test)]
mod refusals_tests;
