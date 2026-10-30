use std::sync::Arc;
use std::time::Duration;

use acting::Acting;
use authorizing::AllowingEverything;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestServer, TestWebSocket};
use clock::FixedClock;
use email::EmailAddress;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_core::{InvitationId, InvitationsCore, Person, Role};
use invitations_rest::TellingInvitations;
use live::Broadcaster;
use live_contract::FrameDTO;
use mailing::FakeMailbox;
use throttling::InMemoryThrottles;

use crate::wiring;
use crate::wiring::ORIGIN;

struct Told {
    invitations: InvitationsCore,
    broadcaster: Broadcaster,
}

fn told() -> Told {
    let broadcaster = Broadcaster::new();

    Told {
        invitations: InvitationsCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryThrottles::default()),
            Arc::new(TellingInvitations::around(
                Arc::new(InMemoryInvitationCatalog::new()),
                broadcaster.clone(),
            )),
            Arc::new(FakeMailbox::default()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(wiring::at(1_700_000_000))),
        ),
        broadcaster,
    }
}

fn as_account(account: &str) -> Acting {
    Acting {
        agent: Agent::Account(AccountId::from(account)),
        operator: false,
    }
}

fn served_to(told: &Told, acting: Acting) -> TestServer {
    TestServer::builder().http_transport().build(
        invitations_rest::live(told.invitations.clone(), told.broadcaster.clone())
            .layer(Extension(acting)),
    )
}

async fn invited_and_listed(told: &Told, to: &str) -> InvitationId {
    let id = told
        .invitations
        .invite_to_project(
            &as_account("account_grace"),
            "project_loom",
            Role::Author,
            to,
            ORIGIN,
        )
        .await
        .expect("an owner may invite")
        .id;
    told.invitations
        .catalogue(&id)
        .await
        .expect("cataloguing should succeed");

    id
}

async fn heard(socket: &mut TestWebSocket) -> String {
    let frame = tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
        .await
        .expect("told while it is fresh");

    serde_json::from_str::<FrameDTO>(&frame)
        .expect("a frame")
        .of
        .expect("a frame says what it is of")
}

#[tokio::test]
async fn an_account_following_its_invitations_is_told_of_each_to_its_address_once_listed() {
    let told = told();
    told.invitations
        .remember_person(&Person {
            account: AccountId::from("account_ada"),
            name: "Ada Lovelace".to_owned(),
            email: EmailAddress::new("ada@example.com").expect("a plain address"),
        })
        .await
        .expect("remembering should succeed");
    let mut socket = served_to(&told, as_account("account_ada"))
        .get_websocket("/invitations/mine/live")
        .await
        .into_websocket()
        .await;

    let elsewhere = invited_and_listed(&told, "hedy@example.com").await;
    let hers = invited_and_listed(&told, "ada@example.com").await;

    assert_ne!(elsewhere, hers);
    assert_eq!(
        heard(&mut socket).await,
        hers.to_string(),
        "nothing of Hedy's invitation reaches Ada"
    );
}

#[tokio::test]
async fn whoever_invitations_know_nothing_of_finds_nothing_to_follow() {
    let told = told();

    served_to(&told, as_account("account_ada"))
        .get_websocket("/invitations/mine/live")
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

fn on_one_socket(told: &Told, agent: Agent) -> TestServer {
    TestServer::builder().http_transport().build(
        live::live_socket(live::Topics::of(vec![Arc::new(
            invitations_rest::InvitationsTopic::new(
                told.invitations.clone(),
                told.broadcaster.clone(),
            ),
        )]))
        .layer(Extension(agent)),
    )
}

async fn subscribed(server: &TestServer, about: serde_json::Value) -> TestWebSocket {
    let mut socket = server.get_websocket("/live").await.into_websocket().await;
    socket
        .send_json(&serde_json::json!({ "do": "subscribe", "id": "bell", "topic": "invitations", "about": about }))
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    socket
}

async fn told_on_one_socket(socket: &mut TestWebSocket) -> live_contract::ToldDTO {
    tokio::time::timeout(Duration::from_secs(1), socket.receive_json())
        .await
        .expect("told while it is fresh")
}

#[tokio::test]
async fn on_the_one_socket_an_account_s_own_invitations_are_told_as_on_their_own() {
    let told = told();
    told.invitations
        .remember_person(&Person {
            account: AccountId::from("account_ada"),
            name: "Ada Lovelace".to_owned(),
            email: EmailAddress::new("ada@example.com").expect("a plain address"),
        })
        .await
        .expect("remembering should succeed");
    let server = on_one_socket(&told, Agent::Account(AccountId::from("account_ada")));
    let mut socket = subscribed(&server, serde_json::json!({ "mine": "true" })).await;

    invited_and_listed(&told, "hedy@example.com").await;
    let hers = invited_and_listed(&told, "ada@example.com").await;

    match told_on_one_socket(&mut socket).await {
        live_contract::ToldDTO::Frame { id, frame } => {
            assert_eq!(id, "bell");
            assert_eq!(
                frame["of"],
                hers.to_string(),
                "nothing of Hedy's reaches Ada"
            );
        }
        other => panic!("a frame of hers, not {other:?}"),
    }
}

#[tokio::test]
async fn on_the_one_socket_the_unknown_and_the_unscoped_find_nothing_there() {
    let told = told();
    told.invitations
        .remember_person(&Person {
            account: AccountId::from("account_ada"),
            name: "Ada Lovelace".to_owned(),
            email: EmailAddress::new("ada@example.com").expect("a plain address"),
        })
        .await
        .expect("remembering should succeed");

    for (agent, about) in [
        (
            Agent::Account(AccountId::from("account_grace")),
            serde_json::json!({ "mine": "true" }),
        ),
        (Agent::Local, serde_json::json!({ "mine": "true" })),
        (
            Agent::Account(AccountId::from("account_ada")),
            serde_json::json!({}),
        ),
    ] {
        let server = on_one_socket(&told, agent);
        let mut socket = subscribed(&server, about).await;

        assert_eq!(
            told_on_one_socket(&mut socket).await,
            live_contract::ToldDTO::Ended {
                id: "bell".to_owned(),
                why: live_contract::ended::NOT_THERE.to_owned(),
            }
        );
    }
}
