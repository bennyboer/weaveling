use std::sync::Arc;

use authorizing::Authorizer;
use clock::Clock;
use eventsourcing::EventStore;
use live::{BroadcastChangesListener, Broadcaster};
use members_catalog::InMemoryMembershipCatalog;
use members_contract::EVERY_MEMBERS;
use members_core::{MembersAuthorizer, MembersCore, MembersEvent, MembershipCatalog};
use members_messaging::{
    CatalogueMembersListener, DisbandMembersOfDeletedProjectListener,
    FoundMembersOfStartedProjectListener, JoinByAcceptedInvitationListener,
    RememberWhoSignedUpListener,
};
use messaging::Publisher;
use outbox::Outbox;
use wiring::{Assembled, Context, EventSourced, Feature, Storage, Unprepared, Wired};

pub const LIVE: &str = "broadcast-membership-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<MembersEvent>>,
    pub catalog: Arc<dyn MembershipCatalog>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> MembersCore {
    MembersCore::new(
        ports.events.clone(),
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
        let EventSourced { events, outbox } =
            EventSourced::in_memory(members_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryMembershipCatalog::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            members_store::codec(),
            members_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(members_catalog::PostgresMembershipCatalog::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            members_catalog::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            members_store::codec(),
            members_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(members_catalog::SqliteMembershipCatalog::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            members_catalog::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        let broadcaster = Broadcaster::new();
        let members = core(ports, context);

        Wired::serving(
            members_rest::router(members.clone())
                .merge(members_rest::live(members.clone(), broadcaster.clone())),
        )
        .listening(vec![
            Arc::new(FoundMembersOfStartedProjectListener::new(members.clone())),
            Arc::new(JoinByAcceptedInvitationListener::new(members.clone())),
            Arc::new(RememberWhoSignedUpListener::new(members.clone())),
            Arc::new(DisbandMembersOfDeletedProjectListener::new(members.clone())),
            Arc::new(CatalogueMembersListener::new(members)),
            Arc::new(BroadcastChangesListener::under(
                LIVE,
                EVERY_MEMBERS,
                members_rest::FOLLOWED,
                broadcaster,
            )),
        ])
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

        ports
            .catalog
            .remember(&ProjectMembers {
                project: project.clone(),
                version: Version::of(1),
                members: vec![Member {
                    account: ada.clone(),
                    role: Role::Owner,
                }],
            })
            .await
            .expect("the list's tables lie beside the events");

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
    fn the_feature_founds_the_members_of_a_started_project_lets_the_invited_join_remembers_who_signed_up_disbands_a_deleted_project_s_lists_every_change_and_tells_it()
     {
        let context = a_context();

        let wired = MembersFeature::wire(&MembersFeature::in_memory(&context), &context);

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
                "catalogue-members",
                "broadcast-membership-changes"
            ]
        );
    }
}

#[cfg(all(test, feature = "postgres"))]
mod postgres_tests {
    use test_harness::PostgresFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_holds_the_list() {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("members").await;

        for _ in 0..2 {
            for schema in MembersFeature::postgres_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        let ports = MembersFeature::on_postgres(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
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

#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests {
    use test_harness::SqliteFixture;

    use super::*;

    #[tokio::test]
    async fn every_part_of_the_schema_lays_down_beside_the_others_again_and_holds_the_list() {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("members").await;

        for _ in 0..2 {
            for schema in MembersFeature::sqlite_schema() {
                schema
                    .run(&pool)
                    .await
                    .expect("each part keeps a ledger of its own, so none mistakes another's 0001");
            }
        }

        let ports = MembersFeature::on_sqlite(pool.clone(), &super::tests::a_context())
            .expect("wiring should succeed");
        super::tests::the_list_keeps_what_it_is_told(&ports).await;
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
