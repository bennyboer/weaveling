use std::sync::Arc;

use eventsourcing::EventStore;
use invitations_core::{InvitationEvent, InvitationService};
use mailing::{LoggingMailer, Mailer};
use outbox::Outbox;
use throttling::{InMemoryThrottles, Throttles};
use wiring::{Assembled, Context, EventSourced, Feature, Storage, Unprepared, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<InvitationEvent>>,
    pub throttles: Arc<dyn Throttles>,
    pub mailer: Arc<dyn Mailer>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn service(ports: &Ports, context: &Context) -> InvitationService {
    InvitationService::new(
        ports.events.clone(),
        ports.throttles.clone(),
        ports.mailer.clone(),
        context.clock.clone(),
    )
}

pub async fn assembled_invitations(
    storage: &Storage,
    context: &Context,
    mailer: Arc<dyn Mailer>,
) -> Result<Assembled, Unprepared> {
    let ports = Ports {
        mailer,
        ..wiring::ports::<InvitationFeature>(storage, context).await?
    };
    let wired = InvitationFeature::wire(&ports, context);

    Ok(Assembled {
        outbox: Some(ports.outbox.clone()),
        routes: wired.routes,
        listeners: wired.listeners,
        settling: wired.settling,
    })
}

pub struct InvitationFeature;

impl Feature for InvitationFeature {
    const NAME: &'static str = "invitations";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(invitations_messaging::message_for, context);

        Ports {
            events,
            throttles: Arc::new(InMemoryThrottles::default()),
            mailer: Arc::new(LoggingMailer),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            invitations_store::codec(),
            invitations_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            throttles: Arc::new(throttling::PostgresThrottles::new(pool)),
            mailer: Arc::new(LoggingMailer),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            throttling::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            invitations_store::codec(),
            invitations_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            throttles: Arc::new(throttling::SqliteThrottles::new(pool)),
            mailer: Arc::new(LoggingMailer),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            throttling::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        Wired::serving(invitations_rest::router(service(ports, context)))
    }
}

#[cfg(all(test, feature = "postgres"))]
mod postgres_tests {
    use test_harness::PostgresFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_and_again() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("invitations").await;

        for _ in 0..2 {
            for schema in InvitationFeature::postgres_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        fixture.cleanup().await;
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests {
    use test_harness::SqliteFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_and_again() {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("invitations").await;

        for _ in 0..2 {
            for schema in InvitationFeature::sqlite_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        fixture.cleanup().await;
    }
}
