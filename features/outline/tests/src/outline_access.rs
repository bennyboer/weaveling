use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Message, RoutingKey};
use outline_core::{OutlinesError, SectionTitle};
use projects_contract::{RESTORED, TRASHED};
use serde_json::{Value, json};

use crate::wiring;
use crate::wiring::Wired;

const THE_LOOM: &str = "project_loom";
const THE_WEFT: &str = "project_weft";

fn a_workbench() -> Wired {
    wiring::wired_with_accounts(Arc::new(FixedClock::new(clock::at(1_000))))
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

fn trashed(project: &str, version: u64) -> Message {
    eventpublishing::published(
        TRASHED,
        "project",
        project,
        version,
        json!({ "version": version, "name": "TRASHED", "until": "2026-12-13T00:00:00Z" }),
        &Agent::System,
        clock::at(1_000),
    )
}

fn restored(project: &str, version: u64) -> Message {
    eventpublishing::published(
        RESTORED,
        "project",
        project,
        version,
        json!({ "version": version, "name": "RESTORED" }),
        &Agent::System,
        clock::at(1_000),
    )
}

async fn hear(wired: &Wired, message: Message) {
    wired
        .remembering_roles
        .handle(&message)
        .await
        .expect("a change to the members is taken in");
}

async fn hear_of_the_trash(wired: &Wired, message: Message) {
    wired
        .remembering_trash
        .handle(&message)
        .await
        .expect("a project put in the trash or restored is taken in");
}

/// Ada founds the loom and Bo joins it to read; Grace founds the weft alone.
async fn the_members_heard(wired: &Wired) {
    hear(wired, founded(THE_LOOM, "account_ada")).await;
    hear(wired, joined(THE_LOOM, 2, "account_bo")).await;
    hear(wired, founded(THE_WEFT, "account_grace")).await;
}

async fn the_loom_s_outline(wired: &Wired) -> String {
    wired
        .outlines
        .open(THE_LOOM, &Agent::System)
        .await
        .expect("the system opens any outline")
        .id
        .to_string()
}

async fn reads(wired: &Wired, outline: &str, named: &str) -> bool {
    wired.outlines.read(outline, &account(named)).await.is_ok()
}

async fn adding_by(wired: &Wired, outline: &str, named: &str) -> Result<(), OutlinesError> {
    wired
        .outlines
        .add(
            outline,
            None,
            None,
            SectionTitle::new("Chapter one").expect("a plain title"),
            None,
            &account(named),
        )
        .await
        .map(|_| ())
}

#[tokio::test]
async fn the_listeners_hear_who_joins_goes_and_changes_role_and_the_trash_but_not_abandoning() {
    // given: the listeners
    let wired = a_workbench();

    // expect: one hears every word on who is a member
    for routing in [FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED] {
        assert!(
            wired
                .remembering_roles
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    // and: not the abandoning that follows the last removal
    assert!(
        !wired
            .remembering_roles
            .hears(&RoutingKey::parse(ABANDONED).expect("a declared key"))
    );
    // and: the other hears a project put in the trash and restored
    for routing in [TRASHED, RESTORED] {
        assert!(
            wired
                .remembering_trash
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn a_role_the_members_tell_is_honored_in_its_project_alone() {
    // given: Ada and Bo in the loom, Grace in the weft, as the members told the outline
    let wired = a_workbench();
    the_members_heard(&wired).await;
    let outline = the_loom_s_outline(&wired).await;

    // expect: Ada and Bo read the loom's outline, and Grace and nobody do not
    assert!(reads(&wired, &outline, "account_ada").await);
    assert!(reads(&wired, &outline, "account_bo").await);
    assert!(!reads(&wired, &outline, "account_grace").await);
    assert!(wired.outlines.read(&outline, &Agent::Local).await.is_err());
    // and: Ada changes it, and Bo is told only an author may
    adding_by(&wired, &outline, "account_ada")
        .await
        .expect("an owner adds a section");
    assert!(matches!(
        adding_by(&wired, &outline, "account_bo").await,
        Err(OutlinesError::NotAnAuthor)
    ));
    // when: Bo becomes an author
    hear(&wired, became_an_author(THE_LOOM, 3, "account_bo")).await;
    // then: he changes it too
    adding_by(&wired, &outline, "account_bo")
        .await
        .expect("an author adds a section");
}

#[tokio::test]
async fn one_who_left_or_whose_project_was_disbanded_reads_the_outline_no_more() {
    // given: Ada and Bo in the loom, and its outline
    let wired = a_workbench();
    the_members_heard(&wired).await;
    let outline = the_loom_s_outline(&wired).await;

    // when: Bo leaves the loom
    hear(&wired, left(THE_LOOM, 3, "account_bo")).await;

    // then: he reads it no more, and Ada still does
    assert!(!reads(&wired, &outline, "account_bo").await);
    assert!(reads(&wired, &outline, "account_ada").await);
    // when: the loom is disbanded
    hear(&wired, disbanded(THE_LOOM, 4)).await;
    // then: Ada reads it no more either
    assert!(!reads(&wired, &outline, "account_ada").await);
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_lets_nobody_in() {
    // given: Ada in the loom, and its outline
    let wired = a_workbench();
    the_members_heard(&wired).await;
    let outline = the_loom_s_outline(&wired).await;

    // when: Grace's leaving of the loom is heard before her joining
    hear(&wired, left(THE_LOOM, 4, "account_grace")).await;
    hear(&wired, joined(THE_LOOM, 3, "account_grace")).await;

    // then: she does not read the outline
    assert!(!reads(&wired, &outline, "account_grace").await);
}

#[tokio::test]
async fn a_project_in_the_trash_is_read_by_nobody_until_restored() {
    // given: Ada and Bo in the loom, and its outline
    let wired = a_workbench();
    the_members_heard(&wired).await;
    let outline = the_loom_s_outline(&wired).await;

    // when: the loom is put in the trash
    hear_of_the_trash(&wired, trashed(THE_LOOM, 4)).await;

    // then: neither reads its outline
    assert!(!reads(&wired, &outline, "account_ada").await);
    assert!(!reads(&wired, &outline, "account_bo").await);
    // when: it is restored
    hear_of_the_trash(&wired, restored(THE_LOOM, 5)).await;
    // then: both read it again
    assert!(reads(&wired, &outline, "account_ada").await);
    assert!(reads(&wired, &outline, "account_bo").await);
}
