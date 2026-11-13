use std::sync::Arc;

use accounts_contract::{BECAME_OPERATOR, DELETED, STOPPED_BEING_OPERATOR};
use assets_contract::{AccountLimitRequest, refused};
use assets_core::{AssetsError, Bound, Limit, Limits, MOST_COUNTED};
use assets_messaging::RememberOperatorsListener;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::Agent;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::bench::{self, Bench};

const A_PORTRAIT: &[u8] = b"the bytes of a portrait";
const BO: &str = "account_bo";

fn told(routing: &str, account: &str, version: u64, name: &str) -> Message {
    eventpublishing::published(
        routing,
        "account",
        account,
        version,
        json!({ "version": version, "name": name }),
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

async fn heard(bench: &Bench, message: Message) {
    Inboxed::new(
        RememberOperatorsListener::new(bench.assets.clone()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_700_000_000,
        ))))),
    )
    .handle(&message)
    .await
    .expect("a change to who runs the instance is taken in");
}

/// Every account held to a megabyte, and Ada running the instance as the accounts told it.
async fn ada_running_the_instance() -> Bench {
    let bench = bench::a_bench_limited(Limits {
        each_account: Some(1 << 20),
        ..Limits::default()
    })
    .await;
    heard(
        &bench,
        told(BECAME_OPERATOR, "account_ada", 3, "BECAME_OPERATOR"),
    )
    .await;

    bench
}

async fn bos_portrait(bench: &Bench) -> Result<(), AssetsError> {
    bench
        .assets
        .upload_as_own(A_PORTRAIT.to_vec(), &bench::bo())
        .await
        .map(|_| ())
}

#[tokio::test]
async fn the_listener_hears_who_becomes_an_operator_stops_or_is_deleted() {
    // given: the listener
    let bench = bench::a_bench().await;
    let listener = Inboxed::new(
        RememberOperatorsListener::new(bench.assets.clone()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_000,
        ))))),
    );

    // expect: it hears every word on who runs the instance
    for routing in [BECAME_OPERATOR, STOPPED_BEING_OPERATOR, DELETED] {
        assert!(
            listener.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn an_operator_gives_an_account_a_limit_of_its_own_which_it_is_held_to() {
    // given: Ada running the instance
    let bench = ada_running_the_instance().await;

    // when: she gives Bo a limit of one byte
    bench
        .assets
        .give_account_limit(BO, 1, &bench::ada())
        .await
        .expect("an operator gives limits");

    // then: his portrait is refused, told his own limit
    let refused = bos_portrait(&bench).await;
    assert!(
        matches!(
            refused,
            Err(AssetsError::LimitReached(Limit {
                bound: Bound::Account,
                most: 1,
                ..
            }))
        ),
        "{refused:?}"
    );
    // when: she takes it back
    bench
        .assets
        .take_back_account_limit(BO, &bench::ada())
        .await
        .expect("an operator takes limits back");
    // then: his portrait is taken under every account's limit
    bos_portrait(&bench)
        .await
        .expect("every account's megabyte takes a portrait");
}

#[tokio::test]
async fn whoever_runs_nothing_neither_gives_nor_takes_back_a_limit() {
    // given: Ada running the instance, and Bo not
    let bench = ada_running_the_instance().await;

    // when: Bo gives himself a gigabyte, and takes back his limit
    let given = bench
        .assets
        .give_account_limit(BO, 1 << 30, &bench::bo())
        .await;
    let taken_back = bench.assets.take_back_account_limit(BO, &bench::bo()).await;

    // then: both are refused, as he runs nothing
    assert!(
        matches!(given, Err(AssetsError::NotAnOperator)),
        "{given:?}"
    );
    assert!(
        matches!(taken_back, Err(AssetsError::NotAnOperator)),
        "{taken_back:?}"
    );
}

#[tokio::test]
async fn an_operator_who_stopped_or_whose_account_was_deleted_gives_no_more_limits() {
    for (routing, name) in [
        (STOPPED_BEING_OPERATOR, "STOPPED_BEING_OPERATOR"),
        (DELETED, "DELETED"),
    ] {
        // given: Ada running the instance
        let bench = ada_running_the_instance().await;

        // when: she stops, or her account is deleted
        heard(&bench, told(routing, "account_ada", 5, name)).await;

        // then: she gives no limit
        let given = bench.assets.give_account_limit(BO, 1, &bench::ada()).await;
        assert!(
            matches!(given, Err(AssetsError::NotAnOperator)),
            "{routing}: {given:?}"
        );
    }
}

#[tokio::test]
async fn a_becoming_heard_after_the_stopping_that_followed_it_lets_nobody_give_limits() {
    // given: Ada's stopping as an operator heard before her becoming one
    let bench = bench::a_bench().await;
    heard(
        &bench,
        told(
            STOPPED_BEING_OPERATOR,
            "account_ada",
            5,
            "STOPPED_BEING_OPERATOR",
        ),
    )
    .await;
    heard(
        &bench,
        told(BECAME_OPERATOR, "account_ada", 3, "BECAME_OPERATOR"),
    )
    .await;

    // when: she gives Bo a limit
    let given = bench.assets.give_account_limit(BO, 1, &bench::ada()).await;

    // then: it is refused, the older word changing nothing
    assert!(
        matches!(given, Err(AssetsError::NotAnOperator)),
        "{given:?}"
    );
}

#[tokio::test]
async fn a_limit_past_what_can_be_counted_is_refused() {
    // given: Ada running the instance
    let bench = ada_running_the_instance().await;

    // when: she gives Bo the most that can be counted, and one byte more
    let most = bench
        .assets
        .give_account_limit(BO, MOST_COUNTED, &bench::ada())
        .await;
    let past = bench
        .assets
        .give_account_limit(BO, MOST_COUNTED + 1, &bench::ada())
        .await;

    // then: the one is given, and the other refused
    assert!(most.is_ok(), "{most:?}");
    assert!(
        matches!(past, Err(AssetsError::LimitUncountable)),
        "{past:?}"
    );
}

#[tokio::test]
async fn the_operator_gives_and_takes_back_a_limit_on_the_instance_s_accounts() {
    // given: Ada running the instance, and the routes for her and for Bo
    let bench = ada_running_the_instance().await;
    let serving = |acting: Agent| {
        TestServer::new(assets_rest::router(bench.assets.clone()).layer(Extension(acting)))
    };
    let (hers, his) = (serving(bench::ada()), serving(bench::bo()));
    let at = format!("/instance/accounts/{BO}/asset-limit");

    // when: Bo gives himself a gigabyte, Ada gives him one byte, and then more than can be counted
    let by_bo = his
        .put(&at)
        .json(&AccountLimitRequest { most: 1 << 30 })
        .await;
    let by_ada = hers.put(&at).json(&AccountLimitRequest { most: 1 }).await;
    let uncountable = hers
        .put(&at)
        .json(&json!({ "most": MOST_COUNTED + 1 }))
        .await;

    // then: Bo is refused as running nothing, Ada's is given, and the uncountable refused
    by_bo.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(by_bo.json::<Value>()["type"], refused::NOT_AN_OPERATOR);
    by_ada.assert_status(StatusCode::NO_CONTENT);
    uncountable.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        uncountable.json::<Value>()["type"],
        refused::LIMIT_UNCOUNTABLE
    );
    // and: Bo is told his own limit
    assert_eq!(his.get("/assets/usage").await.json::<Value>()["most"], 1);
    // when: Ada takes it back
    hers.delete(&at).await.assert_status(StatusCode::NO_CONTENT);
    // then: Bo is told every account's limit again
    assert_eq!(
        his.get("/assets/usage").await.json::<Value>()["most"],
        1 << 20
    );
}
