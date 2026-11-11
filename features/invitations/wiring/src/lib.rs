mod retiring;

use std::sync::Arc;

use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use inbox::{Hearing, InMemoryInbox, Inbox};
use invitations_catalog::{InMemoryInvitationCatalog, InMemoryTokenKeys};
use invitations_core::{
    INVITATIONS_EACH_HOUR, InvitationCatalog, InvitationEvent, InvitationsCore, TokenKeys,
};
use invitations_messaging::{
    AcceptTakenUpInvitationListener, CatalogueInvitationsListener, DeclineInvitationListener,
    ForgetDeletedAccountListener, MailIssuedInvitationListener, RememberProjectNamesListener,
    RememberWhoSignedUpListener, WithdrawIfProjectGoneListener,
    WithdrawInvitationsOfDeletedProjectListener,
};
use invitations_rest::{InvitationsTopic, TellingInvitations};
use live::Broadcaster;
use mailing::{LoggingMailer, Mailer};
use outbox::Outbox;
use throttling::{ForgetOldTries, InMemoryThrottles, Throttles};
use transacting::Transactions;
use wiring::{Assembled, Context, EventSourced, Feature, Storage, Unprepared, Wired};

use crate::retiring::RetireDueInvitations;

pub struct Ports {
    pub events: Arc<dyn EventStore<InvitationEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub keys: Arc<dyn TokenKeys>,
    pub throttles: Arc<dyn Throttles>,
    pub catalog: Arc<dyn InvitationCatalog>,
    pub inbox: Arc<dyn Inbox>,
    pub mailer: Arc<dyn Mailer>,
    pub outbox: Arc<dyn Outbox>,
    pub links_to: Option<String>,
    pub invitations_each_hour: u32,
}

pub fn core(ports: &Ports, context: &Context) -> InvitationsCore {
    let invitations = InvitationsCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.keys.clone(),
        ports.throttles.clone(),
        ports.catalog.clone(),
        ports.mailer.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    );

    let invitations = invitations.capping_invitations(ports.invitations_each_hour);
    match &ports.links_to {
        Some(public_url) => invitations.linking_to(public_url),
        None => invitations,
    }
}

pub async fn assembled_invitations(
    storage: &Storage,
    context: &Context,
    mailer: Arc<dyn Mailer>,
    links_to: Option<String>,
    invitations_each_hour: u32,
) -> Result<Assembled, Unprepared> {
    let ports = Ports {
        mailer,
        links_to,
        invitations_each_hour,
        ..wiring::ports::<InvitationFeature>(storage, context).await?
    };
    let wired = InvitationFeature::wire(&ports, context);

    Ok(Assembled {
        outbox: Some(ports.outbox.clone()),
        routes: wired.routes,
        open_routes: wired.open_routes,
        topics: wired.topics,
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
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(invitations_messaging::message_for, context);

        Ports {
            events,
            transactions,
            keys: Arc::new(InMemoryTokenKeys::new()),
            throttles: Arc::new(InMemoryThrottles::default()),
            catalog: Arc::new(InMemoryInvitationCatalog::new()),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
            mailer: Arc::new(LoggingMailer),
            links_to: None,
            invitations_each_hour: INVITATIONS_EACH_HOUR,
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_postgres(
            &pool,
            invitations_store::codec(),
            invitations_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            keys: Arc::new(invitations_catalog::PostgresTokenKeys::new(pool.clone())),
            throttles: Arc::new(throttling::PostgresThrottles::new(pool.clone())),
            catalog: Arc::new(invitations_catalog::PostgresInvitationCatalog::new(pool)),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
            mailer: Arc::new(LoggingMailer),
            links_to: None,
            invitations_each_hour: INVITATIONS_EACH_HOUR,
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
            inbox::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_sqlite(
            &pool,
            invitations_store::codec(),
            invitations_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            keys: Arc::new(invitations_catalog::SqliteTokenKeys::new(pool.clone())),
            throttles: Arc::new(throttling::SqliteThrottles::new(pool.clone())),
            catalog: Arc::new(invitations_catalog::SqliteInvitationCatalog::new(pool)),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
            mailer: Arc::new(LoggingMailer),
            links_to: None,
            invitations_each_hour: INVITATIONS_EACH_HOUR,
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
            inbox::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
        let broadcaster = Broadcaster::new();
        let invitations = InvitationsCore::new(
            ports.events.clone(),
            ports.transactions.clone(),
            ports.keys.clone(),
            ports.throttles.clone(),
            Arc::new(TellingInvitations::around(
                ports.catalog.clone(),
                broadcaster.clone(),
            )),
            ports.mailer.clone(),
            context.authorizer.clone(),
            context.clock.clone(),
        );
        let invitations = invitations.capping_invitations(ports.invitations_each_hour);
        let invitations = match &ports.links_to {
            Some(public_url) => invitations.linking_to(public_url),
            None => invitations,
        };

        Wired::serving(invitations_rest::router(invitations.clone()))
            .serving_anyone(invitations_rest::router_for_anyone(invitations.clone()))
            .followed_live(vec![Arc::new(InvitationsTopic::new(
                invitations.clone(),
                broadcaster,
            ))])
            .listening(vec![
                Arc::new(
                    hearing.inboxed_effect(MailIssuedInvitationListener::new(invitations.clone())),
                ),
                Arc::new(
                    hearing.inboxed(AcceptTakenUpInvitationListener::new(invitations.clone())),
                ),
                Arc::new(hearing.inboxed(DeclineInvitationListener::new(invitations.clone()))),
                Arc::new(hearing.inboxed(CatalogueInvitationsListener::new(
                    invitations.clone(),
                    ports.outbox.clone(),
                    context.clock.clone(),
                ))),
                Arc::new(hearing.inboxed(WithdrawIfProjectGoneListener::new(invitations.clone()))),
                Arc::new(hearing.inboxed(RememberWhoSignedUpListener::new(invitations.clone()))),
                Arc::new(hearing.inboxed(ForgetDeletedAccountListener::new(invitations.clone()))),
                Arc::new(hearing.inboxed(RememberProjectNamesListener::new(
                    invitations.clone(),
                    ports.outbox.clone(),
                    context.clock.clone(),
                ))),
                Arc::new(WithdrawInvitationsOfDeletedProjectListener::new(
                    invitations.clone(),
                )),
            ])
            .doing_chores(vec![
                Arc::new(RetireDueInvitations::new(invitations.clone())),
                Arc::new(ForgetOldTombstones::new(
                    "forget-old-invitation-tombstones",
                    chores::tombstones_by(move |before, at_most| {
                        let invitations = invitations.clone();
                        async move {
                            invitations
                                .forget_tombstones_laid_before(before, at_most)
                                .await
                        }
                    }),
                    context.clock.clone(),
                )),
                Arc::new(
                    hearing.forgetting("forget-old-invitation-hearings", context.clock.clone()),
                ),
                Arc::new(ForgetOldTries::new(
                    "forget-old-invitation-tries",
                    ports.throttles.clone(),
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

        let mut transaction = ports
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        ports
            .catalog
            .remember(&mut transaction, &summary)
            .await
            .expect("the list's table lies beside the events");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

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
        // given: the feature wired with a tombstone long laid
        let context = a_context();
        let ports = InvitationFeature::in_memory(&context);
        let wired = InvitationFeature::wire(&ports, &context);
        core(&ports, &context)
            .lay_project_tombstone(
                None,
                &ProjectLink::from("project_long_gone"),
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .expect("laying a tombstone should succeed");

        // then: among its chores is one forgetting old tombstones
        let chore = wired
            .chores
            .iter()
            .find(|chore| chore.named() == "forget-old-invitation-tombstones")
            .expect("the feature forgets its tombstones as a chore");

        // and: it forgets the tombstone
        assert_eq!(
            chore
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
    }

    #[test]
    fn the_feature_retires_forgets_its_tombstones_and_forgets_old_hearings_as_chores() {
        // given: what the feature is wired with
        let context = a_context();

        // when: it is wired
        let wired = InvitationFeature::wire(&InvitationFeature::in_memory(&context), &context);

        // then: its chores retire, forget its tombstones and forget old hearings
        assert_eq!(
            wired
                .chores
                .iter()
                .map(|chore| chore.named())
                .collect::<Vec<_>>(),
            [
                "retire-due-invitations",
                "forget-old-invitation-tombstones",
                "forget-old-invitation-hearings",
                "forget-old-invitation-tries"
            ]
        );
    }

    #[tokio::test]
    async fn the_feature_retires_what_is_due_as_a_chore() {
        // given: the feature wired with an invitation due for retiring
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
        let mut transaction = ports
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        ports
            .catalog
            .remember(
                &mut transaction,
                &InvitationSummary {
                    retire_after: OffsetDateTime::UNIX_EPOCH,
                    ..issued.clone()
                },
            )
            .await
            .expect("remembering should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        // then: among its chores is one retiring what is due
        let chore = wired
            .chores
            .iter()
            .find(|chore| chore.named() == "retire-due-invitations")
            .expect("the feature retires as a chore");
        // and: it retires the invitation
        assert_eq!(
            chore
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
        // and: in the system's name
        let told = ports
            .events
            .read_from(&AggregateId::from(&issued.id), KIND, Version::of(1))
            .await
            .expect("reading should succeed");
        assert!(matches!(
            told.last().map(|recorded| &recorded.event),
            Some(InvitationEvent::Snapshotted { retired: true, .. })
        ));
        assert_eq!(
            told.last().map(|recorded| &recorded.metadata.agent),
            Some(&Agent::System),
            "retiring is the system keeping the list tidy, nobody's act"
        );
    }

    #[test]
    fn the_feature_mails_and_hears_its_invitations_taken_up_declined_and_listed_who_signed_up_and_the_projects_names()
     {
        // given: what the feature is wired with
        let context = a_context();

        // when: it is wired
        let wired = InvitationFeature::wire(&InvitationFeature::in_memory(&context), &context);

        // then: it hears its invitations issued, taken up, declined and listed, who signed up and the projects' names
        for named in [
            "mail-issued-invitation",
            "accept-taken-up-invitation",
            "decline-invitation",
            "catalogue-invitation",
            "withdraw-invitation-if-project-gone",
            "invitations-remember-who-signed-up",
            "invitations-forget-deleted-account",
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
        // given: a database of its own
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("invitations").await;

        // when: every part of the schema is laid down twice
        for _ in 0..2 {
            for schema in InvitationFeature::postgres_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        // then: the list keeps what it is told
        let ports = InvitationFeature::on_postgres(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
        // and: in the database
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
        // given: a database of its own
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("invitations").await;

        // when: every part of the schema is laid down twice
        for _ in 0..2 {
            for schema in InvitationFeature::sqlite_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        // then: the list keeps what it is told
        let ports = InvitationFeature::on_sqlite(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
        // and: in the database
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
