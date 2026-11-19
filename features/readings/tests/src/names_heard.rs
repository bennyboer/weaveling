use std::sync::Arc;

use accounts_contract::{DELETED, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use authorizing::AllowingEverything;
use axum::Extension;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message};
use readings_contract::ListedReadingCopyDTO;
use readings_messaging::RememberAccountNamesListener;
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::copying::Copying;

const ADA: &str = "account_ada";

fn told(routing: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "account",
        ADA,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn signed_up(routing: &str, told_as: &str, named: &str) -> Message {
    told(
        routing,
        1,
        json!({
            "version": 1,
            "name": told_as,
            "personal": { "name": named, "email": "ada@example.org" },
        }),
    )
}

fn deleted() -> Message {
    told(DELETED, 9, json!({ "version": 9, "name": "DELETED" }))
}

/// The readings, where everyone may do everything, with a copy Ada made.
async fn a_copy_by_ada() -> (Copying, TestServer) {
    let clock = Arc::new(FixedClock::new(clock::at(1_700_000_000)));
    let copying = crate::copying::a_workbench(
        access::KeptAccess::new(Arc::new(access::InMemoryAccess::new())),
        Arc::new(AllowingEverything),
        clock,
    );
    let as_ = |account: &str| {
        TestServer::new(
            readings_rest::router(copying.readings.clone())
                .layer(Extension(Agent::Account(AccountId::from(account)))),
        )
    };
    as_(ADA)
        .post("/readings/copies")
        .json(&json!({
            "project": "project_loom",
            "name": "Ada's copy",
            "shared": { "kind": "book" },
            "open_until": null,
            "deleted_on": null,
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);
    copying.settle().await;
    let hedy = as_("account_hedy");

    (copying, hedy)
}

async fn maker_listed_for(hedy: &TestServer) -> Option<String> {
    let [copy] = hedy
        .get("/readings/copies?project=project_loom")
        .await
        .json::<Vec<ListedReadingCopyDTO>>()
        .try_into()
        .expect("one copy");

    copy.maker
}

#[tokio::test]
async fn a_copy_names_whoever_made_it_as_they_signed_up_and_no_one_once_their_account_is_deleted() {
    // given: a copy ada made, and the readings' listener of accounts
    let (copying, hedy) = a_copy_by_ada().await;
    let listener = Inboxed::new(
        RememberAccountNamesListener::new(copying.readings.clone()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_700_000_000,
        ))))),
    );

    // expect: before her sign-up is heard, the copy names no one
    assert_eq!(maker_listed_for(&hedy).await, None);

    // when: her sign-up is heard
    listener
        .handle(&signed_up(SIGNED_UP, "SIGNED_UP", "Ada"))
        .await
        .expect("a sign-up should be taken in");

    // then: the copy names her
    assert_eq!(maker_listed_for(&hedy).await.as_deref(), Some("Ada"));

    // when: her account is deleted, and her sign-up is told once more
    listener
        .handle(&deleted())
        .await
        .expect("a deletion should be taken in");
    listener
        .handle(&signed_up(
            SIGNED_UP_BY_INVITATION,
            "SIGNED_UP_BY_INVITATION",
            "Ada",
        ))
        .await
        .expect("a late sign-up should be taken in");

    // then: the copy stays, naming no one, as it belongs to the project
    assert_eq!(maker_listed_for(&hedy).await, None);
}
