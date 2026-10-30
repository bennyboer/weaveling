use std::sync::Arc;

use accounts_contract::{SIGNED_UP, SIGNED_UP_BY_INVITATION};
use clock::FixedClock;
use eventsourcing::{AccountId, InMemoryEventStore, Version};
use messaging::{Handler, Listener, Message, RoutingKey};
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{
    About, NoticeCatalog, NotificationsCore, Person, ProjectLink, ProjectName, Role,
};
use notifications_messaging::{RememberProjectNamesListener, RememberWhoSignedUpListener};
use projects_contract::{DELETED, DETAILS_CHANGED, RENAMED, STARTED};
use registry::InMemoryRegistry;
use serde_json::{Value, json};
use time::OffsetDateTime;

struct Naming {
    catalog: Arc<InMemoryNoticeCatalog>,
    notifications: NotificationsCore,
    people: RememberWhoSignedUpListener,
    projects: RememberProjectNamesListener,
}

fn naming() -> Naming {
    let catalog = Arc::new(InMemoryNoticeCatalog::new());
    let notifications = NotificationsCore::new(
        Arc::new(InMemoryEventStore::new()),
        catalog.clone(),
        Arc::new(InMemoryRegistry::new()),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    Naming {
        people: RememberWhoSignedUpListener::new(notifications.clone()),
        projects: RememberProjectNamesListener::new(notifications.clone()),
        notifications,
        catalog,
    }
}

fn told(key: &str, kind: &str, id: &str, version: u64, body: Value) -> Message {
    Message::opening(
        RoutingKey::parse(key).expect("a declared key"),
        json!({
            "event": body,
            "aggregate": { "id": id, "kind": kind, "version": version },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "1970-01-01T00:00:00Z",
        }),
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn signed_up(key: &str, name: &str) -> Message {
    let event = if key == SIGNED_UP {
        "SIGNED_UP"
    } else {
        "SIGNED_UP_BY_INVITATION"
    };

    told(
        key,
        "account",
        "account_ada",
        1,
        json!({ "version": 0, "name": event, "personal": { "name": name, "email": "ada@example.com" } }),
    )
}

fn project(key: &str, event: &str, version: u64, name: &str) -> Message {
    told(
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
    for key in [SIGNED_UP, SIGNED_UP_BY_INVITATION] {
        let naming = naming();

        naming
            .people
            .take_in(&signed_up(key, "Ada Lovelace"))
            .await
            .expect("remembering should succeed");

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
    let naming = naming();

    for message in [
        project(STARTED, "STARTED", 1, "Shared"),
        project(RENAMED, "RENAMED", 3, "Shared Again"),
        project(RENAMED, "RENAMED", 2, "Between"),
    ] {
        naming
            .projects
            .take_in(&message)
            .await
            .expect("remembering should succeed");
    }

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
    let naming = naming();

    assert!(
        naming
            .people
            .take_in(&told(
                SIGNED_UP,
                "account",
                "account_ada",
                2,
                json!({ "version": 0, "name": "EMAIL_VERIFIED" }),
            ))
            .await
            .is_err()
    );
    assert!(
        naming
            .projects
            .take_in(&told(
                STARTED,
                "project",
                "project_1",
                2,
                json!({ "version": 0, "name": "DETAILS_CHANGED", "language": null, "quotation": null, "description": "" }),
            ))
            .await
            .is_err()
    );
    assert!(person_named(&naming).await.is_empty());
    assert!(project_named(&naming).await.is_empty());
}

#[test]
fn sign_ups_and_projects_named_or_deleted_are_heard() {
    let naming = naming();
    let heard = |listener: &dyn Listener, key: &str| {
        listener.hears(&RoutingKey::parse(key).expect("a declared key"))
    };

    assert!(heard(&naming.people, SIGNED_UP));
    assert!(heard(&naming.people, SIGNED_UP_BY_INVITATION));
    assert!(heard(&naming.projects, STARTED));
    assert!(heard(&naming.projects, RENAMED));
    assert!(heard(&naming.projects, DELETED));
    assert!(!heard(&naming.projects, DETAILS_CHANGED));
}

#[tokio::test]
async fn a_deleted_project_is_named_no_more_and_a_role_changed_in_it_later_notes_nothing() {
    let naming = naming();
    for message in [
        project(STARTED, "STARTED", 1, "Shared"),
        told(
            DELETED,
            "project",
            "project_1",
            2,
            json!({ "version": 0, "name": "DELETED" }),
        ),
        project(RENAMED, "RENAMED", 3, "Heard late"),
    ] {
        naming
            .projects
            .take_in(&message)
            .await
            .expect("each is taken in");
    }

    let noted = naming
        .notifications
        .note(
            "members:project_1@9",
            AccountId::from("account_bo"),
            About::RoleChanged {
                project: ProjectLink::from("project_1"),
                role: Role::Reader,
                by: None,
            },
        )
        .await
        .expect("noting nothing is no failure");

    assert!(project_named(&naming).await.is_empty());
    assert_eq!(
        noted, None,
        "the members' change may be heard after the project's deletion, and tells nothing then"
    );
}
