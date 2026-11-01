use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::{
    Agent, EventStore, FailsAppending, InMemoryEventStore, ServiceError, StoreError,
};
use members_catalog::InMemoryMembershipCatalog;
use members_core::{MembersCore, MembersCoreError, MembersEvent, Role};
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

fn core_over(store: Arc<dyn EventStore<MembersEvent>>) -> MembersCore {
    MembersCore::new(
        store,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryMembershipCatalog::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    )
}

async fn grace_made_an_author_while_losing_races(
    losses: u8,
) -> (Result<(), MembersCoreError>, Option<Role>) {
    let store = Arc::new(InMemoryEventStore::new());
    let members = core_over(store.clone());
    members
        .found("project_1", "account_ada")
        .await
        .expect("founding should succeed");
    members
        .join("project_1", "account_grace", Role::Reader)
        .await
        .expect("joining should succeed");

    let changed = core_over(Arc::new(FailsAppending::losing_races(store, losses)))
        .change_role("project_1", "account_grace", Role::Author, &Agent::Local)
        .await
        .map(|_| ());
    let role = members
        .members("project_1", &Agent::Local)
        .await
        .expect("listing should succeed")
        .into_iter()
        .find(|member| member.account.as_str() == "account_grace")
        .map(|member| member.role);

    (changed, role)
}

#[tokio::test]
async fn a_change_of_role_that_lost_a_race_is_tried_again_and_lands() {
    let (changed, role) = grace_made_an_author_while_losing_races(4).await;

    changed.expect("the fifth try lands");
    assert_eq!(role, Some(Role::Author));
}

#[tokio::test]
async fn a_change_of_role_that_lost_every_race_it_may_try_is_a_conflict() {
    let (changed, role) = grace_made_an_author_while_losing_races(5).await;

    assert!(matches!(
        changed,
        Err(MembersCoreError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    assert_eq!(role, Some(Role::Reader));
}

#[tokio::test]
async fn a_disbanding_that_lost_a_race_is_tried_again_and_lands() {
    let store = Arc::new(InMemoryEventStore::new());
    let members = core_over(store.clone());
    members
        .found("project_1", "account_ada")
        .await
        .expect("founding should succeed");

    core_over(Arc::new(FailsAppending::losing_races(store, 4)))
        .disband("project_1")
        .await
        .expect("the fifth try lands");

    assert!(
        members
            .join("project_1", "account_grace", Role::Reader)
            .await
            .is_ok()
            && members
                .members("project_1", &Agent::Local)
                .await
                .expect("listing should succeed")
                .is_empty(),
        "a disbanded project has no members and takes none"
    );
}
