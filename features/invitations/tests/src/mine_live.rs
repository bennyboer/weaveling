use std::sync::Arc;

use acting::Acting;
use authorizing::AllowingEverything;
use axum::Extension;
use axum_test::TestServer;
use clock::FixedClock;
use email::EmailAddress;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_core::{InvitationId, InvitationsCore, Person, Role};
use invitations_rest::{InvitationChanges, TellingInvitations};
use live::Broadcaster;
use live::testing::Subscriber;
use live_contract::ended;
use mailing::FakeMailbox;
use throttling::InMemoryThrottles;

use crate::wiring;
use crate::wiring::ORIGIN;

struct Told {
    invitations: InvitationsCore,
    broadcaster: Broadcaster<InvitationChanges>,
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

async fn subscribed(server: &TestServer, about: &[(&str, &str)]) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket.subscribe("bell", "invitations", about).await;

    socket
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
    let mut socket = subscribed(&server, &[("mine", "true")]).await;

    invited_and_listed(&told, "hedy@example.com").await;
    let hers = invited_and_listed(&told, "ada@example.com").await;

    assert_eq!(
        socket.frame().await["of"],
        hers.to_string(),
        "nothing of Hedy's reaches Ada"
    );
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

    let mine: &[(&str, &str)] = &[("mine", "true")];
    for (agent, about) in [
        (Agent::Account(AccountId::from("account_grace")), mine),
        (Agent::Local, mine),
        (Agent::Account(AccountId::from("account_ada")), &[]),
    ] {
        let server = on_one_socket(&told, agent);
        let mut socket = subscribed(&server, about).await;

        assert_eq!(socket.ended().await, ended::NOT_THERE);
    }
}
