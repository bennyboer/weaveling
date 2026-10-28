use accounts_contract::{SIGNED_UP, SIGNED_UP_BY_INVITATION};
use email::EmailAddress;
use eventsourcing::AccountId;
use invitations_core::{InvitationCatalog, Person};
use invitations_messaging::RememberWhoSignedUpListener;
use messaging::{Handler, Listener, Message, RoutingKey};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

fn told(routing: &str, event: Value) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key is fine"),
        json!({
            "event": event,
            "aggregate": { "id": "account_ada", "kind": "account", "version": 1 },
            "agent": { "kind": "account", "id": "account_ada" },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn signed_up(named: &str, email: &str) -> Value {
    json!({
        "version": 1,
        "name": named,
        "personal": { "name": "Ada Lovelace", "email": email },
    })
}

async fn known(wired: &Wired) -> Vec<Person> {
    wired
        .catalog
        .people(&[AccountId::from("account_ada")])
        .await
        .expect("asking should succeed")
}

#[tokio::test]
async fn whoever_signed_up_either_way_is_known_by_name_and_address() {
    for (routing, named) in [
        (SIGNED_UP, "SIGNED_UP"),
        (SIGNED_UP_BY_INVITATION, "SIGNED_UP_BY_INVITATION"),
    ] {
        let wired = wiring::wired();
        let listener = RememberWhoSignedUpListener::new(wired.invitations.clone());
        assert!(listener.hears(&RoutingKey::parse(routing).expect("a declared key")));

        listener
            .take_in(&told(routing, signed_up(named, "ada@example.com")))
            .await
            .expect("a sign-up is taken in");

        assert_eq!(
            known(&wired).await,
            vec![Person {
                account: AccountId::from("account_ada"),
                name: "Ada Lovelace".to_owned(),
                email: EmailAddress::new("ada@example.com").expect("a plain address"),
            }]
        );
    }
}

#[tokio::test]
async fn what_is_no_sign_up_or_names_no_address_is_refused_and_teaches_nothing() {
    let wired = wiring::wired();
    let listener = RememberWhoSignedUpListener::new(wired.invitations.clone());

    for unreadable in [
        told(SIGNED_UP, json!({ "version": 1, "name": "EMAIL_VERIFIED" })),
        told(SIGNED_UP, json!({ "version": 1, "name": "SIGNED_UP" })),
        told(SIGNED_UP, signed_up("SIGNED_UP", "not an address")),
    ] {
        assert!(
            listener.take_in(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
    assert!(known(&wired).await.is_empty());
}
