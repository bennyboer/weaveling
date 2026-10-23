use std::sync::Arc;

use axum::Router;
use eventsourcing::EventStore;
use invitations_core::{InvitationEvent, InvitationLinks};
use invitations_links::InMemoryInvitationLinks;
use outbox::Outbox;
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<InvitationEvent>>,
    pub links: Arc<dyn InvitationLinks>,
    pub outbox: Arc<dyn Outbox>,
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
            links: Arc::new(InMemoryInvitationLinks::default()),
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
            links: Arc::new(invitations_links::PostgresInvitationLinks::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            invitations_links::postgres::migrations(),
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
            links: Arc::new(invitations_links::SqliteInvitationLinks::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            invitations_links::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(_ports: &Ports, _context: &Context) -> Wired {
        // TODO 3d-3: the invitations' service and routes
        Wired::serving(Router::new())
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
