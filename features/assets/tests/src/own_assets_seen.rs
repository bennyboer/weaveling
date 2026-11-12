use std::sync::Arc;

use assets_core::{AssetsError, Content};
use assets_messaging::{RememberRolesListener, RememberTrashListener};
use eventsourcing::{Agent, ServiceError};
use inbox::{InMemoryInbox, Inboxed};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::TRASHED;
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::bench::{self, Bench, THE_LOOM, THE_WEFT};

const A_PORTRAIT: &[u8] = b"the bytes of a portrait";

fn heard_by(bench: &Bench) -> Inboxed<RememberRolesListener> {
    Inboxed::new(
        RememberRolesListener::new(bench.assets.clone()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(clock::FixedClock::new(
            clock::at(1_700_000_000),
        )))),
    )
}

fn told(routing: &str, project: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "members",
        project,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
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

fn left(project: &str, version: u64, account: &str) -> Message {
    told(
        LEFT,
        project,
        version,
        json!({ "version": version, "name": "LEFT", "account": account }),
    )
}

async fn hear(listener: &Inboxed<RememberRolesListener>, message: Message) {
    listener
        .handle(&message)
        .await
        .expect("a change to the members is taken in");
}

/// Ada founds the loom and Bo joins it; Grace founds the weft alone.
async fn the_members_heard(listener: &Inboxed<RememberRolesListener>) {
    hear(listener, founded(THE_LOOM, "account_ada")).await;
    hear(listener, joined(THE_LOOM, 2, "account_bo")).await;
    hear(listener, founded(THE_WEFT, "account_grace")).await;
}

async fn adas_portrait(bench: &Bench) -> String {
    let (id, _) = bench
        .assets
        .upload_as_own(A_PORTRAIT.to_vec(), &bench::ada())
        .await
        .expect("an account uploads its own");

    id.to_string()
}

fn is_not_found(seen: &Result<Content, AssetsError>) -> bool {
    matches!(
        seen,
        Err(AssetsError::Events(ServiceError::NotFound { .. }))
    )
}

#[tokio::test]
async fn the_listener_hears_who_joins_goes_and_changes_role_but_not_abandoning() {
    // given: the listener
    let listener = heard_by(&bench::a_bench());

    // expect: it hears every word on who is a member
    for routing in [FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED] {
        assert!(
            listener.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    // and: not the abandoning that follows the last removal
    assert!(!listener.hears(&RoutingKey::parse(ABANDONED).expect("a declared key")));
}

#[tokio::test]
async fn an_account_s_own_portrait_is_seen_by_whoever_shares_a_project_with_it_alone() {
    // given: Ada and Bo in the loom, Grace in the weft, as the members told the assets
    let bench = bench::a_bench();
    the_members_heard(&heard_by(&bench)).await;
    // and: a portrait of Ada's own
    let portrait = adas_portrait(&bench).await;

    // when: Ada, Bo and Grace look at it, and the local agent
    let by_ada = bench.assets.content(&portrait, None, &bench::ada()).await;
    let by_bo = bench.assets.content(&portrait, None, &bench::bo()).await;
    let by_grace = bench.assets.content(&portrait, None, &bench::grace()).await;
    let by_nobody = bench.assets.content(&portrait, None, &Agent::Local).await;

    // then: Ada and Bo see it, and to Grace and nobody it is not there
    assert!(by_ada.is_ok(), "{by_ada:?}");
    assert!(by_bo.is_ok(), "{by_bo:?}");
    assert!(is_not_found(&by_grace), "{by_grace:?}");
    assert!(is_not_found(&by_nobody), "{by_nobody:?}");
}

#[tokio::test]
async fn one_who_left_or_whose_project_was_disbanded_sees_the_portrait_no_more() {
    // given: Ada and Bo in the loom, and a portrait of Ada's own
    let bench = bench::a_bench();
    let listener = heard_by(&bench);
    the_members_heard(&listener).await;
    let portrait = adas_portrait(&bench).await;
    // and: Grace joined the loom too
    hear(&listener, joined(THE_LOOM, 3, "account_grace")).await;
    assert!(
        bench
            .assets
            .content(&portrait, None, &bench::grace())
            .await
            .is_ok()
    );

    // when: Grace leaves the loom
    hear(&listener, left(THE_LOOM, 4, "account_grace")).await;

    // then: she sees it no more, and Bo still does
    assert!(is_not_found(
        &bench.assets.content(&portrait, None, &bench::grace()).await
    ));
    assert!(
        bench
            .assets
            .content(&portrait, None, &bench::bo())
            .await
            .is_ok()
    );
    // when: the loom is disbanded
    hear(
        &listener,
        told(
            DISBANDED,
            THE_LOOM,
            5,
            json!({ "version": 5, "name": "DISBANDED" }),
        ),
    )
    .await;

    // then: Bo sees it no more, and Ada her own still
    assert!(is_not_found(
        &bench.assets.content(&portrait, None, &bench::bo()).await
    ));
    assert!(
        bench
            .assets
            .content(&portrait, None, &bench::ada())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_shows_nothing() {
    // given: Ada in the loom, and a portrait of Ada's own
    let bench = bench::a_bench();
    let listener = heard_by(&bench);
    the_members_heard(&listener).await;
    let portrait = adas_portrait(&bench).await;

    // when: Grace's leaving of the loom is heard before her joining
    hear(&listener, left(THE_LOOM, 4, "account_grace")).await;
    hear(&listener, joined(THE_LOOM, 3, "account_grace")).await;

    // then: she does not see the portrait
    assert!(is_not_found(
        &bench.assets.content(&portrait, None, &bench::grace()).await
    ));
}

#[tokio::test]
async fn a_project_in_the_trash_is_shared_by_nobody_until_restored() {
    // given: Ada and Bo in the loom, and a portrait of Ada's own
    let bench = bench::a_bench();
    the_members_heard(&heard_by(&bench)).await;
    let portrait = adas_portrait(&bench).await;
    let trash = Inboxed::new(
        RememberTrashListener::new(bench.assets.clone()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(clock::FixedClock::new(
            clock::at(1_700_000_000),
        )))),
    );

    // when: the loom is put in the trash
    trash
        .handle(&eventpublishing::published(
            TRASHED,
            "project",
            THE_LOOM,
            4,
            json!({ "version": 4, "name": "TRASHED", "until": "2026-12-13T00:00:00Z" }),
            &Agent::System,
            clock::at(1_700_000_000),
        ))
        .await
        .expect("a project put in the trash is taken in");

    // then: Bo sees the portrait no more
    assert!(is_not_found(
        &bench.assets.content(&portrait, None, &bench::bo()).await
    ));
}
