use std::sync::Arc;

use accounts_contract::{SIGNED_UP, SIGNED_UP_BY_INVITATION};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, Version};
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{
    About, NoticeCatalog, NotificationsCore, Person, ProjectLink, ProjectName, Role,
};
use notifications_messaging::{RememberProjectNamesListener, RememberWhoSignedUpListener};
use projects_contract::{DELETED, DETAILS_CHANGED, RENAMED, STARTED};
use serde_json::{Value, json};
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

struct Naming {
    catalog: Arc<InMemoryNoticeCatalog>,
    notifications: NotificationsCore,
    people: Inboxed<RememberWhoSignedUpListener>,
    projects: Inboxed<RememberProjectNamesListener>,
}

fn a_workbench() -> Naming {
    let catalog = Arc::new(InMemoryNoticeCatalog::new());
    let notifications = NotificationsCore::new(
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryTransactions::new()),
        catalog.clone(),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    Naming {
        people: Inboxed::new(
            RememberWhoSignedUpListener::new(notifications.clone()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                OffsetDateTime::UNIX_EPOCH,
            )))),
        ),
        projects: Inboxed::new(
            RememberProjectNamesListener::new(notifications.clone()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                OffsetDateTime::UNIX_EPOCH,
            )))),
        ),
        notifications,
        catalog,
    }
}

fn a_message(key: &str, kind: &str, id: &str, version: u64, body: Value) -> Message {
    eventpublishing::published(
        key,
        kind,
        id,
        version,
        body,
        &Agent::System,
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn a_sign_up(key: &str, name: &str) -> Message {
    let event = if key == SIGNED_UP {
        "SIGNED_UP"
    } else {
        "SIGNED_UP_BY_INVITATION"
    };

    a_message(
        key,
        "account",
        "account_ada",
        1,
        json!({ "version": 0, "name": event, "personal": { "name": name, "email": "ada@example.com" } }),
    )
}

fn a_project_change(key: &str, event: &str, version: u64, name: &str) -> Message {
    a_message(
        key,
        "project",
        "project_1",
        version,
        json!({ "version": 0, "name": event, "project_name": name }),
    )
}

async fn person_named(naming: &Naming) -> Vec<Person> {
    naming
        .catalog
        .people(&[AccountId::from("account_ada")])
        .await
        .expect("asking should succeed")
}

async fn project_named(naming: &Naming) -> Vec<ProjectName> {
    naming
        .catalog
        .project_names(&[ProjectLink::from("project_1")])
        .await
        .expect("asking should succeed")
}

#[tokio::test]
async fn whoever_signs_up_either_way_is_known_by_their_name_and_no_address() {
    // given: either way of signing up
    for key in [SIGNED_UP, SIGNED_UP_BY_INVITATION] {
        // and: the listeners remembering names
        let naming = a_workbench();

        // when: a sign-up is heard
        naming
            .people
            .handle(&a_sign_up(key, "Ada Lovelace"))
            .await
            .expect("remembering should succeed");

        // then: whoever signed up is known by name and no address
        assert_eq!(
            person_named(&naming).await,
            vec![Person {
                account: AccountId::from("account_ada"),
                name: "Ada Lovelace".to_owned(),
            }]
        );
    }
}

#[tokio::test]
async fn a_project_is_known_by_the_name_it_started_with_and_then_by_its_newest() {
    // given: the listeners remembering names
    let naming = a_workbench();

    // when: a project's start, a rename and an older rename heard late are heard
    for message in [
        a_project_change(STARTED, "STARTED", 1, "Shared"),
        a_project_change(RENAMED, "RENAMED", 3, "Shared Again"),
        a_project_change(RENAMED, "RENAMED", 2, "Between"),
    ] {
        naming
            .projects
            .handle(&message)
            .await
            .expect("remembering should succeed");
    }

    // then: it is known by the newest name
    assert_eq!(
        project_named(&naming).await,
        vec![ProjectName {
            project: ProjectLink::from("project_1"),
            version: Version::of(3),
            name: "Shared Again".to_owned(),
        }]
    );
}

#[tokio::test]
async fn what_tells_no_name_belongs_in_dead_letters() {
    // given: the listeners remembering names
    let naming = a_workbench();

    // then: a sign-up telling no name is refused
    assert!(
        naming
            .people
            .handle(&a_message(
                SIGNED_UP,
                "account",
                "account_ada",
                2,
                json!({ "version": 0, "name": "EMAIL_VERIFIED" }),
            ))
            .await
            .is_err()
    );
    // and: a project's change telling no name is refused
    assert!(
        naming
            .projects
            .handle(&a_message(
                STARTED,
                "project",
                "project_1",
                2,
                json!({ "version": 0, "name": "DETAILS_CHANGED", "language": null, "quotation": null, "description": "" }),
            ))
            .await
            .is_err()
    );
    // and: nobody is named
    assert!(person_named(&naming).await.is_empty());
    // and: no project is named
    assert!(project_named(&naming).await.is_empty());
}

#[test]
fn sign_ups_and_projects_named_or_deleted_are_heard() {
    // given: the listeners remembering names
    let naming = a_workbench();
    let heard = |listener: &dyn Listener, key: &str| {
        listener.hears(&RoutingKey::parse(key).expect("a declared key"))
    };

    // then: they hear sign-ups either way
    assert!(heard(&naming.people, SIGNED_UP));
    assert!(heard(&naming.people, SIGNED_UP_BY_INVITATION));
    // and: projects started, renamed and deleted
    assert!(heard(&naming.projects, STARTED));
    assert!(heard(&naming.projects, RENAMED));
    assert!(heard(&naming.projects, DELETED));
    // and: not a project's details changed
    assert!(!heard(&naming.projects, DETAILS_CHANGED));
}

#[tokio::test]
async fn a_deleted_project_is_named_no_more_and_a_role_changed_in_it_later_notes_nothing() {
    // given: the listeners remembering names
    let naming = a_workbench();
    // when: a project's start, its deletion and a rename heard late are heard
    for message in [
        a_project_change(STARTED, "STARTED", 1, "Shared"),
        a_message(
            DELETED,
            "project",
            "project_1",
            2,
            json!({ "version": 0, "name": "DELETED" }),
        ),
        a_project_change(RENAMED, "RENAMED", 3, "Heard late"),
    ] {
        naming
            .projects
            .handle(&message)
            .await
            .expect("each is taken in");
    }

    // and: a role changed in it is noted
    let noted = naming
        .notifications
        .note(
            None,
            AccountId::from("account_bo"),
            About::RoleChanged {
                project: ProjectLink::from("project_1"),
                role: Role::Reader,
                by: None,
            },
        )
        .await
        .expect("noting nothing is no failure");

    // then: it is named no more
    assert!(project_named(&naming).await.is_empty());
    // and: nothing is noted
    assert_eq!(
        noted, None,
        "the members' change may be heard after the project's deletion, and tells nothing then"
    );
}
