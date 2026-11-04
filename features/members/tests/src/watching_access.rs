use std::sync::Arc;
use std::time::Duration;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use members_catalog::InMemoryMembershipCatalog;
use members_core::{MembersAuthorizer, MembersCore, ProjectLink, Role};
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

const PROJECT: &str = "project_1";

async fn heard(watch: &mut AccessWatch) -> bool {
    tokio::time::timeout(Duration::from_millis(50), watch.changed())
        .await
        .is_ok()
}

fn grace() -> Agent {
    Agent::Account(AccountId::from("account_grace"))
}

#[tokio::test]
async fn a_change_to_the_members_is_told_once_the_list_shows_it() {
    // given: a project with an owner and an author, as the list shows it
    let catalog = Arc::new(InMemoryMembershipCatalog::new());
    let authorizer = Arc::new(MembersAuthorizer::new(catalog.clone()));
    let members = MembersCore::new(
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryTransactions::new()),
        catalog,
        authorizer.clone(),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    let project = ProjectLink::from(PROJECT);
    members
        .found(None, PROJECT, "account_ada")
        .await
        .expect("founding should succeed");
    members
        .join(None, PROJECT, "account_grace", Role::Author)
        .await
        .expect("joining should succeed");
    members
        .catalogue(None, &project)
        .await
        .expect("cataloguing should succeed");
    // and: access to it and to another project watched
    let mut watch = authorizer.watch_access(PROJECT);
    let mut elsewhere = authorizer.watch_access("project_other");

    // when: the author is removed
    members
        .remove(
            PROJECT,
            "account_grace",
            &Agent::Account(AccountId::from("account_ada")),
        )
        .await
        .expect("an owner may remove an author");
    // then: no change of access is heard yet
    assert!(
        !heard(&mut watch).await,
        "the list is not told yet, so asking again would still let her in"
    );

    // when: the list shows the removal
    members
        .catalogue(None, &project)
        .await
        .expect("cataloguing should succeed");

    // then: the change of access is heard
    assert!(heard(&mut watch).await);
    // and: the author may no longer read the project
    assert!(
        !authorizer
            .may(&grace(), Act::Read, PROJECT)
            .await
            .expect("asking should succeed"),
        "whoever asks again on hearing it is answered by the list as it is now"
    );
    // and: nothing is heard of the other project
    assert!(!heard(&mut elsewhere).await);
}
