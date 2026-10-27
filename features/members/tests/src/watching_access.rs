use std::sync::Arc;
use std::time::Duration;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use members_catalog::InMemoryMembershipCatalog;
use members_core::{MembersAuthorizer, MembersCore, ProjectLink, Role};
use time::OffsetDateTime;

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
    let catalog = Arc::new(InMemoryMembershipCatalog::new());
    let authorizer = Arc::new(MembersAuthorizer::new(catalog.clone()));
    let members = MembersCore::new(
        Arc::new(InMemoryEventStore::new()),
        catalog,
        authorizer.clone(),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    let project = ProjectLink::from(PROJECT);
    members
        .found(PROJECT, "account_ada")
        .await
        .expect("founding should succeed");
    members
        .join(PROJECT, "account_grace", Role::Author)
        .await
        .expect("joining should succeed");
    members
        .catalogue(&project)
        .await
        .expect("cataloguing should succeed");
    let mut watch = authorizer.watch_access(PROJECT);
    let mut elsewhere = authorizer.watch_access("project_other");

    members
        .remove(
            PROJECT,
            "account_grace",
            &Agent::Account(AccountId::from("account_ada")),
        )
        .await
        .expect("an owner may remove an author");
    assert!(
        !heard(&mut watch).await,
        "the list is not told yet, so asking again would still let her in"
    );
    members
        .catalogue(&project)
        .await
        .expect("cataloguing should succeed");

    assert!(heard(&mut watch).await);
    assert!(
        !authorizer
            .may(&grace(), Act::Read, PROJECT)
            .await
            .expect("asking should succeed"),
        "whoever asks again on hearing it is answered by the list as it is now"
    );
    assert!(!heard(&mut elsewhere).await);
}
