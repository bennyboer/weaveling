mod retiring;
mod tombstones;

use std::sync::Arc;

use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_core::{InvitationCatalog, InvitationEvent, InvitationsCore};
use invitations_messaging::{
    AcceptTakenUpInvitationListener, CatalogueInvitationsListener, DeclineInvitationListener,
    RememberProjectNamesListener, RememberWhoSignedUpListener,
    WithdrawInvitationsOfDeletedProjectListener,
};
use invitations_rest::TellingInvitations;
use live::Broadcaster;
use mailing::{LoggingMailer, Mailer};
use outbox::Outbox;
use throttling::{InMemoryThrottles, Throttles};
use wiring::{Assembled, Context, EventSourced, Feature, Storage, Unprepared, Wired};

use crate::retiring::RetireDueInvitations;
use crate::tombstones::InvitationTombstones;

pub struct Ports {
    pub events: Arc<dyn EventStore<InvitationEvent>>,
    pub throttles: Arc<dyn Throttles>,
    pub catalog: Arc<dyn InvitationCatalog>,
    pub mailer: Arc<dyn Mailer>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> InvitationsCore {
    InvitationsCore::new(
        ports.events.clone(),
        ports.throttles.clone(),
        ports.catalog.clone(),
        ports.mailer.clone(),
        context.authorizer.clone(),
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
        chores: wired.chores,
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
            catalog: Arc::new(InMemoryInvitationCatalog::new()),
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
            throttles: Arc::new(throttling::PostgresThrottles::new(pool.clone())),
            catalog: Arc::new(invitations_catalog::PostgresInvitationCatalog::new(pool)),
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
            invitations_catalog::postgres::migrations(),
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
            throttles: Arc::new(throttling::SqliteThrottles::new(pool.clone())),
            catalog: Arc::new(invitations_catalog::SqliteInvitationCatalog::new(pool)),
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
            invitations_catalog::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        let broadcaster = Broadcaster::new();
        let invitations = InvitationsCore::new(
            ports.events.clone(),
            ports.throttles.clone(),
            Arc::new(TellingInvitations::around(
                ports.catalog.clone(),
                broadcaster.clone(),
            )),
            ports.mailer.clone(),
            context.authorizer.clone(),
            context.clock.clone(),
        );

        Wired::serving(
            invitations_rest::router(invitations.clone())
                .merge(invitations_rest::live(invitations.clone(), broadcaster)),
        )
        .listening(vec![
            Arc::new(AcceptTakenUpInvitationListener::new(invitations.clone())),
            Arc::new(DeclineInvitationListener::new(invitations.clone())),
            Arc::new(CatalogueInvitationsListener::new(invitations.clone())),
            Arc::new(RememberWhoSignedUpListener::new(invitations.clone())),
            Arc::new(RememberProjectNamesListener::new(invitations.clone())),
            Arc::new(WithdrawInvitationsOfDeletedProjectListener::new(
                invitations.clone(),
            )),
        ])
        .doing_chores(vec![
            Arc::new(RetireDueInvitations::new(invitations.clone())),
            Arc::new(ForgetOldTombstones::new(
                "forget-old-invitation-tombstones",
                Arc::new(InvitationTombstones::of(invitations)),
                context.clock.clone(),
            )),
        ])
    }
}

#[cfg(test)]
mod tests {
    use acting::Acting;
    use authorizing::AllowingEverything;
    use clock::SystemClock;
    use email::EmailAddress;
    use eventsourcing::Version;
    use eventsourcing::{Agent, AggregateId};
    use invitations_core::{
        InvitationId, InvitationSummary, InvitationTokenDigest, KIND, Outcome, ProjectLink, Scope,
    };
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use time::OffsetDateTime;

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

    pub(super) async fn the_list_keeps_what_it_is_told(ports: &Ports) {
        let summary = InvitationSummary {
            id: InvitationId::generate(OffsetDateTime::UNIX_EPOCH),
            version: Version::of(1),
            to: EmailAddress::new("ada@example.com").expect("a plain address"),
            digest: InvitationTokenDigest::kept("ab12".to_owned()),
            issued_by: None,
            issued_at: OffsetDateTime::UNIX_EPOCH,
            until: OffsetDateTime::UNIX_EPOCH,
            outcome: Outcome::Open,
            retire_after: OffsetDateTime::UNIX_EPOCH,
            scope: Scope::Instance,
        };

        ports
            .catalog
            .remember(&summary)
            .await
            .expect("the list's table lies beside the events");

        assert_eq!(
            ports
                .catalog
                .listed_after(None, 10)
                .await
                .expect("listing should succeed"),
            vec![summary]
        );
    }

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_as_a_chore() {
        let context = a_context();
        let ports = InvitationFeature::in_memory(&context);
        let wired = InvitationFeature::wire(&ports, &context);
        ports
            .catalog
            .lay_project_tombstone(
                &ProjectLink::from("project_long_gone"),
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .expect("laying a tombstone should succeed");

        let chore = wired
            .chores
            .iter()
            .find(|chore| chore.named() == "forget-old-invitation-tombstones")
            .expect("the feature forgets its tombstones as a chore");

        assert_eq!(
            chore
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
    }

    #[tokio::test]
    async fn the_feature_retires_what_is_due_as_a_chore() {
        let context = a_context();
        let ports = InvitationFeature::in_memory(&context);
        let wired = InvitationFeature::wire(&ports, &context);
        let issued = core(&ports, &context)
            .invite_to_instance(
                &Acting {
                    agent: Agent::Account("account_grace".into()),
                    operator: true,
                },
                "ada@example.com",
                "https://weaveling.example.com",
            )
            .await
            .expect("an operator may invite");
        ports
            .catalog
            .remember(&InvitationSummary {
                retire_after: OffsetDateTime::UNIX_EPOCH,
                ..issued.clone()
            })
            .await
            .expect("remembering should succeed");

        let chore = wired
            .chores
            .iter()
            .find(|chore| chore.named() == "retire-due-invitations")
            .expect("the feature retires as a chore");
        assert_eq!(
            chore
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
        let told = ports
            .events
            .read_from(&AggregateId::from(&issued.id), KIND, Version::of(1))
            .await
            .expect("reading should succeed");
        assert!(matches!(
            told.last().map(|recorded| &recorded.event),
            Some(InvitationEvent::Retired)
        ));
        assert_eq!(
            told.last().map(|recorded| &recorded.metadata.agent),
            Some(&Agent::System),
            "retiring is the system keeping the list tidy, nobody's act"
        );
    }

    #[test]
    fn the_feature_hears_its_invitations_taken_up_declined_and_listed_who_signed_up_and_the_projects_names()
     {
        let context = a_context();

        let wired = InvitationFeature::wire(&InvitationFeature::in_memory(&context), &context);

        for named in [
            "accept-taken-up-invitation",
            "decline-invitation",
            "catalogue-invitation",
            "invitations-remember-who-signed-up",
            "remember-project-names",
            "withdraw-invitations-of-deleted-project",
        ] {
            assert!(
                wired
                    .listeners
                    .iter()
                    .any(|listener| listener.named().as_str() == named),
                "{named}"
            );
        }
    }
}

#[cfg(all(test, feature = "postgres"))]
mod postgres_tests {
    use test_harness::PostgresFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_holds_the_list() {
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

        let ports = InvitationFeature::on_postgres(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
        let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM invitation_summaries")
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
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_holds_the_list() {
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

        let ports = InvitationFeature::on_sqlite(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
        let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM invitation_summaries")
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
