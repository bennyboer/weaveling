use accounts_contract::{SIGNED_UP, SIGNED_UP_BY_INVITATION};
use email::EmailAddress;
use eventsourcing::AccountId;
use invitations_core::{InvitationCatalog, Person};
use invitations_messaging::RememberWhoSignedUpListener;
use messaging::{Listener, Message, RoutingKey};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

fn a_message(routing: &str, event: Value) -> Message {
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

fn a_sign_up(named: &str, email: &str) -> Value {
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
    // given: either way of signing up
    for (routing, named) in [
        (SIGNED_UP, "SIGNED_UP"),
        (SIGNED_UP_BY_INVITATION, "SIGNED_UP_BY_INVITATION"),
    ] {
        // and: the listener remembering who signed up
        let wired = wiring::a_workbench();
        let listener = wiring::inboxed(RememberWhoSignedUpListener::new(wired.invitations.clone()));
        // then: it hears that way
        assert!(listener.hears(&RoutingKey::parse(routing).expect("a declared key")));

        // when: a sign-up is heard
        listener
            .handle(&a_message(routing, a_sign_up(named, "ada@example.com")))
            .await
            .expect("a sign-up is taken in");

        // then: whoever signed up is known by name and address
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
    // given: the listener remembering who signed up
    let wired = wiring::a_workbench();
    let listener = wiring::inboxed(RememberWhoSignedUpListener::new(wired.invitations.clone()));

    // when: what is no sign-up or names no address is heard
    for unreadable in [
        a_message(SIGNED_UP, json!({ "version": 1, "name": "EMAIL_VERIFIED" })),
        a_message(SIGNED_UP, json!({ "version": 1, "name": "SIGNED_UP" })),
        a_message(SIGNED_UP, a_sign_up("SIGNED_UP", "not an address")),
    ] {
        // then: it is refused
        assert!(
            listener.handle(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
    // and: nobody is known
    assert!(known(&wired).await.is_empty());
}
