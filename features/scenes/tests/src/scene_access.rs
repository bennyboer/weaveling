use std::sync::Arc;
use std::time::Duration;

use belonging::InMemoryBelongings;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::{RESTORED, TRASHED};
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{Access, Scene, ScenesCore};
use scenes_messaging::{RememberRolesListener, RememberTrashListener};
use scenes_store::InMemorySceneStore;
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::kept::Kept;

const THE_LOOM: &str = "project_loom";
const THE_WEFT: &str = "project_weft";

struct Heard {
    scenes: ScenesCore,
    roles: Inboxed<RememberRolesListener>,
    trash: Inboxed<RememberTrashListener>,
}

fn inboxed<L: KeptHandler>(listener: L) -> Inboxed<L> {
    Inboxed::new(
        listener,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_000,
        ))))),
    )
}

fn a_workbench() -> Heard {
    let kept = Kept::default();
    let scenes = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
        kept.access.clone(),
        kept.authorizer(),
        Arc::new(FixedClock::new(clock::at(1_000))),
    );

    Heard {
        roles: inboxed(RememberRolesListener::new(scenes.clone())),
        trash: inboxed(RememberTrashListener::new(scenes.clone())),
        scenes,
    }
}

fn account(named: &str) -> Agent {
    Agent::Account(AccountId::from(named))
}

fn told(routing: &str, project: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "members",
        project,
        version,
        event,
        &Agent::System,
        clock::at(1_000),
    )
}

fn founded(project: &str, owner: &str) -> Message {
    told(
        FOUNDED,
        project,
        1,
        json!({ "version": 1, "name": "FOUNDED", "owner": owner }),
    )
}

fn joined(project: &str, version: u64, account: &str) -> Message {
    told(
        JOINED,
        project,
        version,
        json!({ "version": version, "name": "JOINED", "account": account, "role": role::READER }),
    )
}

fn became_an_author(project: &str, version: u64, account: &str) -> Message {
    told(
        ROLE_CHANGED,
        project,
        version,
        json!({ "version": version, "name": "ROLE_CHANGED", "account": account, "to": role::AUTHOR }),
    )
}

fn removed(project: &str, version: u64, account: &str) -> Message {
    told(
        REMOVED,
        project,
        version,
        json!({ "version": version, "name": "REMOVED", "account": account }),
    )
}

fn left(project: &str, version: u64, account: &str) -> Message {
    told(
        LEFT,
        project,
        version,
        json!({ "version": version, "name": "LEFT", "account": account }),
    )
}

fn disbanded(project: &str, version: u64) -> Message {
    told(
        DISBANDED,
        project,
        version,
        json!({ "version": version, "name": "DISBANDED" }),
    )
}

fn of_the_project(routing: &str, project: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "project",
        project,
        version,
        event,
        &Agent::System,
        clock::at(1_000),
    )
}

async fn hear(heard: &Heard, message: Message) {
    heard
        .roles
        .handle(&message)
        .await
        .expect("a change to the members is taken in");
}

async fn hear_of_the_trash(heard: &Heard, message: Message) {
    heard
        .trash
        .handle(&message)
        .await
        .expect("a project put in the trash or restored is taken in");
}

/// Ada founds the loom and Bo joins it to read; Grace founds the weft alone.
async fn the_members_heard(heard: &Heard) {
    hear(heard, founded(THE_LOOM, "account_ada")).await;
    hear(heard, joined(THE_LOOM, 2, "account_bo")).await;
    hear(heard, founded(THE_WEFT, "account_grace")).await;
}

async fn a_scene_of_the_loom(heard: &Heard) -> Scene {
    heard
        .scenes
        .create(THE_LOOM, &Agent::System)
        .await
        .expect("the system creates a scene anywhere")
}

async fn access_of(heard: &Heard, scene: &Scene, named: &str) -> Access {
    heard
        .scenes
        .access(scene, &account(named))
        .await
        .expect("asking should succeed")
}

#[tokio::test]
async fn the_listeners_hear_who_joins_goes_and_changes_role_and_the_trash_but_not_abandoning() {
    // given: the listeners
    let heard = a_workbench();

    // expect: one hears every word on who is a member
    for routing in [FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED] {
        assert!(
            heard
                .roles
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    // and: not the abandoning that follows the last removal
    assert!(
        !heard
            .roles
            .hears(&RoutingKey::parse(ABANDONED).expect("a declared key"))
    );
    // and: the other hears a project put in the trash and restored
    for routing in [TRASHED, RESTORED] {
        assert!(
            heard
                .trash
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn a_role_the_members_tell_is_honored_in_its_project_alone() {
    // given: Ada and Bo in the loom, Grace in the weft, as the members told the scenes
    let heard = a_workbench();
    the_members_heard(&heard).await;
    let scene = a_scene_of_the_loom(&heard).await;

    // expect: Ada writes the loom's scene, Bo reads it, and Grace is a stranger to it
    assert_eq!(
        access_of(&heard, &scene, "account_ada").await,
        Access::Author
    );
    assert_eq!(
        access_of(&heard, &scene, "account_bo").await,
        Access::Reader
    );
    assert_eq!(
        access_of(&heard, &scene, "account_grace").await,
        Access::Stranger
    );
    // when: Bo becomes an author
    hear(&heard, became_an_author(THE_LOOM, 3, "account_bo")).await;
    // then: he writes it too
    assert_eq!(
        access_of(&heard, &scene, "account_bo").await,
        Access::Author
    );
}

#[tokio::test]
async fn one_who_left_was_removed_or_whose_project_was_disbanded_reads_the_scene_no_more() {
    // given: Ada and Bo in the loom, Grace joined too, and a scene of it
    let heard = a_workbench();
    the_members_heard(&heard).await;
    hear(&heard, joined(THE_LOOM, 3, "account_grace")).await;
    let scene = a_scene_of_the_loom(&heard).await;

    // when: Bo leaves the loom and Grace is removed from it
    hear(&heard, left(THE_LOOM, 4, "account_bo")).await;
    hear(&heard, removed(THE_LOOM, 5, "account_grace")).await;

    // then: neither reads it any more, and Ada still writes it
    assert_eq!(
        access_of(&heard, &scene, "account_bo").await,
        Access::Stranger
    );
    assert_eq!(
        access_of(&heard, &scene, "account_grace").await,
        Access::Stranger
    );
    assert_eq!(
        access_of(&heard, &scene, "account_ada").await,
        Access::Author
    );
    // when: the loom is disbanded
    hear(&heard, disbanded(THE_LOOM, 6)).await;
    // then: Ada is a stranger to it too
    assert_eq!(
        access_of(&heard, &scene, "account_ada").await,
        Access::Stranger
    );
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_lets_nobody_in() {
    // given: Ada in the loom, and a scene of it
    let heard = a_workbench();
    the_members_heard(&heard).await;
    let scene = a_scene_of_the_loom(&heard).await;

    // when: Grace's leaving of the loom is heard before her joining
    hear(&heard, left(THE_LOOM, 4, "account_grace")).await;
    hear(&heard, joined(THE_LOOM, 3, "account_grace")).await;

    // then: she is a stranger to the scene
    assert_eq!(
        access_of(&heard, &scene, "account_grace").await,
        Access::Stranger
    );
}

#[tokio::test]
async fn a_project_in_the_trash_is_read_by_nobody_until_restored() {
    // given: Ada and Bo in the loom, and a scene of it
    let heard = a_workbench();
    the_members_heard(&heard).await;
    let scene = a_scene_of_the_loom(&heard).await;

    // when: the loom is put in the trash
    hear_of_the_trash(
        &heard,
        of_the_project(
            TRASHED,
            THE_LOOM,
            4,
            json!({ "version": 4, "name": "TRASHED", "until": "2026-12-13T00:00:00Z" }),
        ),
    )
    .await;

    // then: both are strangers to its scene
    for named in ["account_ada", "account_bo"] {
        assert_eq!(access_of(&heard, &scene, named).await, Access::Stranger);
    }
    // when: it is restored
    hear_of_the_trash(
        &heard,
        of_the_project(
            RESTORED,
            THE_LOOM,
            5,
            json!({ "version": 5, "name": "RESTORED" }),
        ),
    )
    .await;
    // then: each has their role in it again
    assert_eq!(
        access_of(&heard, &scene, "account_ada").await,
        Access::Author
    );
    assert_eq!(
        access_of(&heard, &scene, "account_bo").await,
        Access::Reader
    );
}

#[tokio::test]
async fn whoever_follows_a_project_is_told_once_a_leaving_is_taken_in() {
    // given: Ada and Bo in the loom, and someone following it
    let heard = a_workbench();
    the_members_heard(&heard).await;
    let mut watch = heard.scenes.watch_access(THE_LOOM);

    // when: Bo's leaving is heard
    hear(&heard, left(THE_LOOM, 3, "account_bo")).await;

    // then: the follower is told
    tokio::time::timeout(Duration::from_secs(1), watch.changed())
        .await
        .expect("the change is told once taken in");
}
