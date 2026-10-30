use std::sync::Arc;

use eventsourcing::EventStore;
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{NoticeCatalog, NoticeEvent, NotificationsCore};
use notifications_messaging::{
    CatalogueNoticeListener, NoteRoleChangedListener, RememberProjectNamesListener,
    RememberWhoSignedUpListener,
};
use outbox::Outbox;
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<NoticeEvent>>,
    pub catalog: Arc<dyn NoticeCatalog>,
    pub registry: Arc<dyn Registry>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> NotificationsCore {
    NotificationsCore::new(
        ports.events.clone(),
        ports.catalog.clone(),
        ports.registry.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let notifications = core(ports, context);

    Wired::serving(notifications_rest::router(notifications.clone())).listening(vec![
        Arc::new(NoteRoleChangedListener::new(notifications.clone())),
        Arc::new(CatalogueNoticeListener::new(notifications.clone())),
        Arc::new(RememberWhoSignedUpListener::new(notifications.clone())),
        Arc::new(RememberProjectNamesListener::new(notifications)),
    ])
}

pub struct NotificationsFeature;

impl Feature for NotificationsFeature {
    const NAME: &'static str = "notifications";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(notifications_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryNoticeCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            notifications_store::codec(),
            notifications_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(notifications_catalog::PostgresNoticeCatalog::new(
                pool.clone(),
            )),
            registry: Arc::new(registry::PostgresRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            notifications_catalog::postgres::migrations(),
            registry::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            notifications_store::codec(),
            notifications_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(notifications_catalog::SqliteNoticeCatalog::new(
                pool.clone(),
            )),
            registry: Arc::new(registry::SqliteRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            notifications_catalog::sqlite::migrations(),
            registry::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}

#[cfg(test)]
mod tests {
    use authorizing::AllowingEverything;
    use clock::SystemClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};

    use super::*;

    pub(super) fn a_context() -> Context {
        Context {
            clock: Arc::new(SystemClock),
            publisher: Arc::new(InProcessDispatcher::queueing_to(Arc::new(
                InMemoryDeliveries::new(),
            ))),
            authorizer: Arc::new(AllowingEverything),
        }
    }

    #[test]
    fn the_feature_notes_a_changed_role_lists_every_notice_and_remembers_names() {
        let context = a_context();

        let wired =
            NotificationsFeature::wire(&NotificationsFeature::in_memory(&context), &context);

        let named: Vec<String> = wired
            .listeners
            .iter()
            .map(|listener| listener.named().as_str().to_owned())
            .collect();
        assert_eq!(
            named,
            [
                "note-role-changed",
                "catalogue-notice",
                "notifications-remember-who-signed-up",
                "notifications-remember-project-names"
            ]
        );
        assert!(wired.topics.is_empty());
        assert!(wired.chores.is_empty());
    }

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    pub(super) async fn the_list_keeps_a_noted_notice(ports: &Ports) {
        use eventsourcing::AccountId;
        use notifications_core::{About, ProjectLink, Role};

        let notifications = core(ports, &a_context());
        let bo = AccountId::from("account_bo");

        let notice = notifications
            .note(
                "members:project_1@4",
                bo.clone(),
                About::RoleChanged {
                    project: ProjectLink::from("project_1"),
                    role: Role::Author,
                    by: Some(AccountId::from("account_ada")),
                },
            )
            .await
            .expect("noting should succeed")
            .expect("its project is not deleted");
        notifications
            .catalogue(&notice)
            .await
            .expect("cataloguing should succeed");

        let open = ports
            .catalog
            .open_for(&bo)
            .await
            .expect("listing should succeed");
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, notice);
    }
}

#[cfg(all(test, feature = "postgres"))]
mod postgres_tests {
    use test_harness::PostgresFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_keeps_the_notices() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("notifications").await;

        for _ in 0..2 {
            for schema in NotificationsFeature::postgres_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        let ports = NotificationsFeature::on_postgres(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_a_noted_notice(&ports).await;
        let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM notices")
            .fetch_one(&pool)
            .await
            .expect("counting should succeed");
        assert_eq!(
            kept, 1,
            "the list is kept in the feature's database, not in memory"
        );

        fixture.cleanup().await;
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests {
    use test_harness::SqliteFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_keeps_the_notices() {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("notifications").await;

        for _ in 0..2 {
            for schema in NotificationsFeature::sqlite_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        let ports = NotificationsFeature::on_sqlite(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_a_noted_notice(&ports).await;
        let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM notices")
            .fetch_one(&pool)
            .await
            .expect("counting should succeed");
        assert_eq!(
            kept, 1,
            "the list is kept in the feature's database, not in memory"
        );

        fixture.cleanup().await;
    }
}
