use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, EventStore, FailsAppending, InMemoryEventStore, ServiceError, StoreError,
};
use inbox::{InMemoryInbox, Inboxed};
use members_catalog::InMemoryMembershipCatalog;
use members_core::{MembersCore, MembersCoreError, MembersError, MembersEvent, Role};
use members_messaging::DisbandMembersOfDeletedProjectListener;
use messaging::Listener;
use projects_contract::DELETED;
use serde_json::json;
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

async fn making_grace_an_author_while_losing_races(
    losses: u8,
) -> (Result<(), MembersCoreError>, Option<Role>) {
    let store = Arc::new(InMemoryEventStore::new());
    let members = core_over(store.clone());
    members
        .found(None, "project_1", "account_ada")
        .await
        .expect("founding should succeed");
    members
        .join(None, "project_1", "account_grace", Role::Reader)
        .await
        .expect("joining should succeed");

    let changed = core_over(Arc::new(FailsAppending::losing_races(store, losses)))
        .change_role(
            "project_1",
            "account_grace",
            Role::Author,
            None,
            &Agent::Local,
        )
        .await
        .map(|_| ());
    let role = members
        .members("project_1", &Agent::Local)
        .await
        .expect("listing should succeed")
        .state
        .into_iter()
        .find(|member| member.account.as_str() == "account_grace")
        .map(|member| member.role);

    (changed, role)
}

#[tokio::test]
async fn a_change_of_role_that_lost_a_race_is_tried_again_and_lands() {
    // when: a reader is made an author while four races are lost
    let (changed, role) = making_grace_an_author_while_losing_races(4).await;

    // then: the change lands
    changed.expect("the fifth try lands");
    assert_eq!(role, Some(Role::Author));
}

#[tokio::test]
async fn a_change_of_role_that_lost_every_race_it_may_try_is_a_conflict() {
    // when: a reader is made an author while every race it may try is lost
    let (changed, role) =
        making_grace_an_author_while_losing_races(transacting::retrying::TRIES).await;

    // then: it is refused as outdated
    assert!(matches!(
        changed,
        Err(MembersCoreError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: they stay a reader
    assert_eq!(role, Some(Role::Reader));
}

#[tokio::test]
async fn a_disbanding_that_lost_a_race_is_refused_and_lands_when_it_is_offered_again() {
    // given: a project's members founded
    let store = Arc::new(InMemoryEventStore::new());
    let members = core_over(store.clone());
    members
        .found(None, "project_1", "account_ada")
        .await
        .expect("founding should succeed");
    // and: a listener that loses one race
    let listener = Inboxed::new(
        DisbandMembersOfDeletedProjectListener::new(core_over(Arc::new(
            FailsAppending::losing_races(store, 1),
        ))),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
            OffsetDateTime::UNIX_EPOCH,
        )))),
    );
    // and: the project's deletion
    let deleted = eventpublishing::published(
        DELETED,
        "project",
        "project_1",
        4,
        json!({ "version": 0, "name": "DELETED" }),
        &Agent::Account(AccountId::from("account_ada")),
        OffsetDateTime::UNIX_EPOCH,
    );

    // expect: hearing it the first time is refused
    listener
        .handle(&deleted)
        .await
        .expect_err("a lost race is refused, for the message to be offered again");
    // and: hearing it again lands
    listener
        .handle(&deleted)
        .await
        .expect("offered again, it lands");
    // and: the members are disbanded and take nobody in
    assert!(
        members
            .join(None, "project_1", "account_grace", Role::Reader)
            .await
            .is_ok()
            && members
                .members("project_1", &Agent::Local)
                .await
                .expect("listing should succeed")
                .state
                .is_empty(),
        "a disbanded project has no members and takes none"
    );
}

#[tokio::test]
async fn an_owner_removed_cannot_make_themselves_owner_again_while_the_list_still_says_they_are() {
    // given: an owner removed by another
    let members = core_over(Arc::new(InMemoryEventStore::new()));
    members
        .found(None, "project_1", "account_ada")
        .await
        .expect("founding should succeed");
    members
        .join(None, "project_1", "account_grace", Role::Owner)
        .await
        .expect("joining should succeed");
    members
        .remove(
            "project_1",
            "account_grace",
            None,
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("an owner may remove another");

    // when: they make themselves owner again
    let regained = members
        .change_role(
            "project_1",
            "account_grace",
            Role::Owner,
            None,
            &Agent::Account("account_grace".into()),
        )
        .await;

    // then: it is refused as by no owner
    assert!(
        matches!(
            regained,
            Err(MembersCoreError::Events(ServiceError::Refused(
                MembersError::NotAnOwner
            )))
        ),
        "an authorizer that still lets them in, as a list behind its events would, is not enough: {regained:?}"
    );
}

#[tokio::test]
async fn an_owner_stepped_down_removes_nobody() {
    // given: an owner stepped down by another
    let members = core_over(Arc::new(InMemoryEventStore::new()));
    members
        .found(None, "project_1", "account_ada")
        .await
        .expect("founding should succeed");
    members
        .join(None, "project_1", "account_grace", Role::Owner)
        .await
        .expect("joining should succeed");
    members
        .change_role(
            "project_1",
            "account_grace",
            Role::Reader,
            None,
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("an owner may step another down");

    // when: they remove the other
    let removing = members
        .remove(
            "project_1",
            "account_ada",
            None,
            &Agent::Account("account_grace".into()),
        )
        .await;

    // then: it is refused as by no owner
    assert!(matches!(
        removing,
        Err(MembersCoreError::Events(ServiceError::Refused(
            MembersError::NotAnOwner
        )))
    ));
}
