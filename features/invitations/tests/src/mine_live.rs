use std::sync::Arc;

use acting::Acting;
use authorizing::AllowingEverything;
use axum::Extension;
use axum_test::TestServer;
use clock::FixedClock;
use email::EmailAddress;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, Version};
use invitations_catalog::{InMemoryInvitationCatalog, InMemoryTokenKeys};
use invitations_core::{
    InvitationCatalog, InvitationId, InvitationsCore, Person, ProjectLink, ProjectName, Role,
};
use invitations_rest::{InvitationChanges, TellingInvitations};
use live::Broadcaster;
use live::testing::Subscriber;
use live_contract::ended;
use mailing::FakeMailbox;
use throttling::InMemoryThrottles;
use transacting::{InMemoryTransactions, Transactions};

use crate::wiring;
use crate::wiring::ORIGIN;

struct Told {
    invitations: InvitationsCore,
    broadcaster: Broadcaster<InvitationChanges>,
}

fn a_workbench() -> Told {
    let broadcaster = Broadcaster::new();

    Told {
        invitations: InvitationsCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryTokenKeys::new()),
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
        .catalogue(None, &id)
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
    // given: ada known and subscribed to her own invitations on the one socket
    let told = a_workbench();
    told.invitations
        .remember_person(
            None,
            &Person {
                account: AccountId::from("account_ada"),
                name: "Ada Lovelace".to_owned(),
                email: EmailAddress::new("ada@example.com").expect("a plain address"),
            },
        )
        .await
        .expect("remembering should succeed");
    let server = on_one_socket(&told, Agent::Account(AccountId::from("account_ada")));
    let mut socket = subscribed(&server, &[("mine", "true")]).await;

    // when: an invitation to another and one to her are listed
    invited_and_listed(&told, "hedy@example.com").await;
    let hers = invited_and_listed(&told, "ada@example.com").await;

    // then: she is told of hers alone
    assert_eq!(
        socket.frame().await["of"],
        hers.to_string(),
        "nothing of Hedy's reaches Ada"
    );
}

#[tokio::test]
async fn an_invitee_is_told_when_the_project_they_are_invited_into_is_renamed_trashed_or_restored()
{
    // given: ada known and subscribed to her own invitations on the one socket
    let told = a_workbench();
    told.invitations
        .remember_person(
            None,
            &Person {
                account: AccountId::from("account_ada"),
                name: "Ada Lovelace".to_owned(),
                email: EmailAddress::new("ada@example.com").expect("a plain address"),
            },
        )
        .await
        .expect("remembering should succeed");
    let server = on_one_socket(&told, Agent::Account(AccountId::from("account_ada")));
    let mut socket = subscribed(&server, &[("mine", "true")]).await;
    // and: invited into a project
    invited_and_listed(&told, "ada@example.com").await;
    socket.frame().await;
    let loom = ProjectLink::from("project_loom");

    // when: the project is renamed, trashed and restored
    told.invitations
        .remember_project_name(
            None,
            &ProjectName {
                project: loom.clone(),
                version: Version::of(2),
                name: "The Loom".to_owned(),
            },
        )
        .await
        .expect("remembering should succeed");
    let renamed = socket.frame().await;
    told.invitations
        .remember_trash(None, &loom, Version::of(3), true)
        .await
        .expect("remembering should succeed");
    let trashed = socket.frame().await;
    told.invitations
        .remember_trash(None, &loom, Version::of(4), false)
        .await
        .expect("remembering should succeed");
    let restored = socket.frame().await;
    // and: another project is trashed
    told.invitations
        .remember_trash(
            None,
            &ProjectLink::from("project_other"),
            Version::of(2),
            true,
        )
        .await
        .expect("remembering should succeed");

    // then: she is told of each change to hers
    for frame in [renamed, trashed, restored] {
        assert_eq!(frame["of"], "project_loom");
    }
    // and: of nothing else
    assert!(
        socket.silent().await,
        "a project Ada is not invited into tells her nothing"
    );
}

#[tokio::test]
async fn on_the_one_socket_the_unknown_and_the_unscoped_find_nothing_there() {
    // given: ada known
    let told = a_workbench();
    told.invitations
        .remember_person(
            None,
            &Person {
                account: AccountId::from("account_ada"),
                name: "Ada Lovelace".to_owned(),
                email: EmailAddress::new("ada@example.com").expect("a plain address"),
            },
        )
        .await
        .expect("remembering should succeed");

    // when: someone unknown, the local agent, and ada asking for no scope each subscribe on the one socket
    let mine: &[(&str, &str)] = &[("mine", "true")];
    for (agent, about) in [
        (Agent::Account(AccountId::from("account_grace")), mine),
        (Agent::Local, mine),
        (Agent::Account(AccountId::from("account_ada")), &[]),
    ] {
        let server = on_one_socket(&told, agent);
        let mut socket = subscribed(&server, about).await;

        // then: each finds nothing there
        assert_eq!(socket.ended().await, ended::NOT_THERE);
    }
}

#[tokio::test]
async fn nobody_is_told_of_an_invitation_remembered_in_a_transaction_that_is_dropped() {
    // given: ada known and subscribed to her own invitations on the one socket
    let told = a_workbench();
    told.invitations
        .remember_person(
            None,
            &Person {
                account: AccountId::from("account_ada"),
                name: "Ada Lovelace".to_owned(),
                email: EmailAddress::new("ada@example.com").expect("a plain address"),
            },
        )
        .await
        .expect("remembering should succeed");
    let telling = TellingInvitations::around(
        Arc::new(InMemoryInvitationCatalog::new()),
        told.broadcaster.clone(),
    );
    let server = on_one_socket(&told, Agent::Account(AccountId::from("account_ada")));
    let mut socket = subscribed(&server, &[("mine", "true")]).await;
    // and: an invitation to her remembered in a transaction
    let issued = told
        .invitations
        .invite_to_instance(&wiring::an_operator(), "ada@example.com", ORIGIN)
        .await
        .expect("an operator may invite");
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    telling
        .remember(&mut transaction, &issued)
        .await
        .expect("remembering should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: she is told nothing
    assert!(socket.silent().await);
}
