use std::sync::Arc;

use authorizing::Authorizer;
use clock::Clock;
use eventsourcing::EventStore;
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::{BroadcastChangesListener, Broadcaster};
use members_catalog::InMemoryMembershipCatalog;
use members_contract::EVERY_MEMBERS;
use members_core::{MembersAuthorizer, MembersCore, MembersEvent, MembershipCatalog};
use members_messaging::{
    CatalogueMembersListener, DisbandMembersOfDeletedProjectListener,
    FoundMembersOfStartedProjectListener, JoinByAcceptedInvitationListener,
    RememberTrashedProjectsListener, RememberWhoSignedUpListener,
};
use messaging::Publisher;
use outbox::Outbox;
use transacting::Transactions;
use wiring::{Assembled, Context, EventSourced, Feature, Storage, Unprepared, Wired};

pub const LIVE: &str = "broadcast-membership-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<MembersEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn MembershipCatalog>,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> MembersCore {
    MembersCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.catalog.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub struct AssembledMembers {
    pub assembled: Assembled,
    pub authorizer: Arc<dyn Authorizer>,
}

pub async fn assembled_members(
    storage: &Storage,
    clock: Arc<dyn Clock>,
    publisher: Arc<dyn Publisher>,
) -> Result<AssembledMembers, Unprepared> {
    let knowing_nobody = Context {
        clock: clock.clone(),
        publisher: publisher.clone(),
        authorizer: Arc::new(MembersAuthorizer::new(Arc::new(
            InMemoryMembershipCatalog::new(),
        ))),
    };
    let ports = wiring::ports::<MembersFeature>(storage, &knowing_nobody).await?;
    let authorizer: Arc<dyn Authorizer> = Arc::new(MembersAuthorizer::new(ports.catalog.clone()));
    let wired = MembersFeature::wire(
        &ports,
        &Context {
            clock,
            publisher,
            authorizer: authorizer.clone(),
        },
    );

    Ok(AssembledMembers {
        assembled: Assembled {
            outbox: Some(ports.outbox.clone()),
            routes: wired.routes,
            open_routes: wired.open_routes,
            topics: wired.topics,
            listeners: wired.listeners,
            settling: wired.settling,
            chores: wired.chores,
        },
        authorizer,
    })
}

pub struct MembersFeature;

impl Feature for MembersFeature {
    const NAME: &'static str = "members";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(members_messaging::message_for, context);

        Ports {
            events,
            transactions,
            catalog: Arc::new(InMemoryMembershipCatalog::new()),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
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
            members_store::codec(),
            members_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(members_catalog::PostgresMembershipCatalog::new(pool)),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            members_catalog::postgres::migrations(),
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
            members_store::codec(),
            members_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(members_catalog::SqliteMembershipCatalog::new(pool)),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            members_catalog::sqlite::migrations(),
            inbox::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
        let broadcaster = Broadcaster::new();
        let members = core(ports, context);

        Wired::serving(members_rest::router(members.clone()))
            .followed_live(vec![Arc::new(members_rest::MembersTopic::new(
                members.clone(),
                broadcaster.clone(),
            ))])
            .listening(vec![
                Arc::new(
                    hearing.inboxed(FoundMembersOfStartedProjectListener::new(members.clone())),
                ),
                Arc::new(hearing.inboxed(JoinByAcceptedInvitationListener::new(members.clone()))),
                Arc::new(hearing.inboxed(RememberWhoSignedUpListener::new(members.clone()))),
                Arc::new(
                    hearing.inboxed(DisbandMembersOfDeletedProjectListener::new(members.clone())),
                ),
                Arc::new(hearing.inboxed(RememberTrashedProjectsListener::new(members.clone()))),
                Arc::new(hearing.inboxed(CatalogueMembersListener::new(members))),
                Arc::new(BroadcastChangesListener::new(
                    LIVE,
                    EVERY_MEMBERS,
                    broadcaster,
                    |_| vec![members_rest::MembershipChanges],
                )),
            ])
            .doing_chores(vec![Arc::new(
                hearing.forgetting("forget-old-member-hearings", context.clock.clone()),
            )])
    }
}

#[cfg(test)]
mod tests {
    use authorizing::AllowingEverything;
    use clock::SystemClock;
    use eventsourcing::{AccountId, Version};
    use members_core::{Member, ProjectLink, ProjectMembers, Role};
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

    pub(super) async fn the_list_keeps_what_it_is_told(ports: &Ports) {
        let ada = AccountId::from("account_ada");
        let project = ProjectLink::from("project_1");
        let mut transaction = ports
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");

        ports
            .catalog
            .remember(
                &mut transaction,
                &ProjectMembers {
                    project: project.clone(),
                    version: Version::of(1),
                    members: vec![Member {
                        account: ada.clone(),
                        role: Role::Owner,
                    }],
                },
            )
            .await
            .expect("the list's tables lie beside the events");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        assert_eq!(
            ports
                .catalog
                .role_of(&project, &ada)
                .await
                .expect("asking should succeed"),
            Some(Role::Owner)
        );
    }

    #[test]
    fn the_feature_founds_the_members_of_a_started_project_lets_the_invited_join_remembers_who_signed_up_disbands_a_deleted_project_s_lists_every_change_tells_it_and_forgets_old_hearings()
     {
        // given: a context
        let context = a_context();

        // when: the feature is wired in memory
        let wired = MembersFeature::wire(&MembersFeature::in_memory(&context), &context);

        // then: it listens as it should, in that order
        let named: Vec<String> = wired
            .listeners
            .iter()
            .map(|listener| listener.named().as_str().to_owned())
            .collect();
        assert_eq!(
            named,
            [
                "found-members-of-started-project",
                "join-by-accepted-invitation",
                "remember-who-signed-up",
                "disband-members-of-deleted-project",
                "members-remember-trashed-projects",
                "catalogue-members",
                "broadcast-membership-changes"
            ]
        );
        // and: it forgets old hearings
        assert_eq!(
            wired
                .chores
                .iter()
                .map(|chore| chore.named())
                .collect::<Vec<_>>(),
            ["forget-old-member-hearings"]
        );
    }
}

#[cfg(all(test, feature = "postgres"))]
mod postgres_tests {
    use eventsourcing::{AccountId, Agent};
    use members_core::Person;
    use test_harness::PostgresFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_holds_the_list() {
        // given: a database
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("members").await;

        // when: every part of the schema is laid down twice
        for _ in 0..2 {
            for schema in MembersFeature::postgres_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        // then: the feature wired on it keeps what its list is told
        let ports = MembersFeature::on_postgres(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
        // and: keeps it in the database
        let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships")
            .fetch_one(&pool)
            .await
            .expect("counting should succeed");
        assert_eq!(
            kept, 1,
            "the list is kept in the feature's database, not in memory"
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn founding_a_project_already_founded_leaves_the_transaction_it_shares_whole() {
        // given: the feature on a database
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("members").await;
        for schema in MembersFeature::postgres_schema() {
            schema.run(&pool).await.expect("the schema lays down");
        }
        let ports = MembersFeature::on_postgres(pool, &super::tests::a_context())
            .expect("wiring should succeed");
        let members = core(&ports, &super::tests::a_context());

        // when: a project is founded twice in one transaction
        let mut transaction = ports
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        for _ in 0..2 {
            members
                .found(Some(&mut transaction), "project_1", "account_ada")
                .await
                .expect("founding again is no failure");
        }
        let finished = transaction.finish().await;

        // then: the transaction commits
        assert!(finished.is_ok(), "{finished:?}");
        // and: the project has its owner
        assert_eq!(
            members
                .members("project_1", &Agent::System)
                .await
                .expect("a founded project has members")
                .state
                .len(),
            1,
            "a second founding that fails inside the transaction would take the first and everything beside it down with it"
        );

        fixture.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_founding_that_loses_a_race_never_answers_success_over_a_lost_transaction() {
        // given: the feature on a database
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("members").await;
        for schema in MembersFeature::postgres_schema() {
            schema.run(&pool).await.expect("the schema lays down");
        }
        let ports = MembersFeature::on_postgres(pool, &super::tests::a_context())
            .expect("wiring should succeed");
        let members = core(&ports, &super::tests::a_context());
        // and: a project being founded in a transaction not yet committed
        let mut first = ports
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        members
            .found(Some(&mut first), "project_1", "account_ada")
            .await
            .expect("the first founding lands");

        // when: it is founded again in another transaction at the same time
        let racing = tokio::spawn({
            let (members, transactions) = (members.clone(), ports.transactions.clone());
            async move {
                let mut second = transactions
                    .begin()
                    .await
                    .expect("beginning should succeed");
                let founded = members
                    .found(Some(&mut second), "project_1", "account_ada")
                    .await;
                let beside = match &founded {
                    Ok(()) => Some(
                        members
                            .remember_person(
                                Some(&mut second),
                                &Person {
                                    account: AccountId::from("account_ada"),
                                    name: "Ada".to_owned(),
                                    email: "ada@example.com".to_owned(),
                                },
                            )
                            .await,
                    ),
                    Err(_) => None,
                };

                (founded, beside)
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        first.finish().await.expect("the first commits");
        let (founded, beside) = racing.await.expect("the second finishes");

        // then: the second either fails, to be tried again, or what is written beside it lands
        if founded.is_ok() {
            assert!(
                matches!(beside, Some(Ok(()))),
                "a founding answered as done over a transaction the database already gave up loses whatever is written beside it: {beside:?}"
            );
        }

        fixture.cleanup().await;
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests {
    use test_harness::SqliteFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_holds_the_list() {
        // given: a database
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("members").await;

        // when: every part of the schema is laid down twice
        for _ in 0..2 {
            for schema in MembersFeature::sqlite_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        // then: the feature wired on it keeps what its list is told
        let ports = MembersFeature::on_sqlite(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
        // and: keeps it in the database
        let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships")
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
