use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::Clock;
use eventsourcing::Agent;
use inbox::Inboxed;
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_contract::{ACCEPTED, DECLINED, ISSUED, RETIRED, WITHDRAWN};
use invitations_core::{
    DeclineReason, InvitationCatalog, InvitationId, InvitationSummary, InvitationsCore, Outcome,
};
use invitations_messaging::CatalogueInvitationsListener;
use messaging::{Listener, Message, RoutingKey};
use serde_json::json;
use throttling::InMemoryThrottles;
use time::Duration;
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

fn told(routing: &str, invitation: &InvitationId) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "WHATEVER" },
            "aggregate": { "id": invitation.to_string(), "kind": "invitation", "version": 1 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        wiring::at(1_700_000_000),
    )
}

struct Listing {
    wired: Wired,
    catalog: Arc<InMemoryInvitationCatalog>,
    invitations: InvitationsCore,
    listener: Inboxed<CatalogueInvitationsListener>,
}

fn listing() -> Listing {
    let wired = wiring::wired();
    let catalog = Arc::new(InMemoryInvitationCatalog::new());
    let invitations = InvitationsCore::new(
        wired.store.clone(),
        Arc::new(InMemoryTransactions::new()),
        wired.keys.clone(),
        Arc::new(InMemoryThrottles::default()),
        catalog.clone(),
        wired.mailbox.clone(),
        Arc::new(AllowingEverything),
        wired.clock.clone(),
    );

    Listing {
        listener: wiring::inboxed(CatalogueInvitationsListener::new(
            invitations.clone(),
            wired.outbox.clone(),
            wired.clock.clone(),
        )),
        wired,
        catalog,
        invitations,
    }
}

impl Listing {
    async fn invited(&self, to: &str) -> InvitationId {
        self.wired
            .invitations
            .invite_to_instance(&wiring::an_operator(), to, ORIGIN)
            .await
            .expect("an operator may invite")
            .id
    }

    async fn listed(&self) -> Vec<InvitationSummary> {
        self.catalog
            .listed_after(None, 100)
            .await
            .expect("listing should succeed")
    }
}

#[tokio::test]
async fn an_invitation_issued_is_listed_as_it_stands() {
    let listing = listing();
    let id = listing.invited(ADA).await;

    listing
        .listener
        .handle(&told(ISSUED, &id))
        .await
        .expect("cataloguing should succeed");

    let listed = listing.listed().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].to.as_str(), ADA);
    assert_eq!(listed[0].issued_at, wiring::at(1_700_000_000));
    assert_eq!(
        listed[0].until,
        wiring::at(1_700_000_000) + Duration::days(7)
    );
    assert_eq!(listed[0].outcome, Outcome::Open);
}

#[tokio::test]
async fn an_issue_heard_after_its_invitation_was_taken_up_lists_nothing() {
    let listing = listing();
    let id = listing.invited(ADA).await;
    listing
        .wired
        .invitations
        .taken_up(
            None,
            &id,
            wiring::at(1_700_000_100),
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("an open invitation is accepted");

    listing
        .listener
        .handle(&told(ISSUED, &id))
        .await
        .expect("cataloguing should succeed");

    assert!(
        listing.listed().await.is_empty(),
        "the list reads how the invitation stands now, not what the message was about"
    );
}

#[tokio::test]
async fn a_declined_invitation_stays_listed_and_says_why() {
    let listing = listing();
    let id = listing.invited(ADA).await;
    listing
        .wired
        .invitations
        .declined(None, &id, DeclineReason::AddressHasAccount)
        .await
        .expect("an open invitation is declined");

    listing
        .listener
        .handle(&told(DECLINED, &id))
        .await
        .expect("cataloguing should succeed");

    assert_eq!(
        listing.listed().await[0].outcome,
        Outcome::Declined(DeclineReason::AddressHasAccount)
    );
}

#[tokio::test]
async fn every_change_to_an_invitation_is_heard() {
    let listing = listing();

    for routing in [ISSUED, ACCEPTED, WITHDRAWN, DECLINED] {
        assert!(
            listing
                .listener
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    assert!(
        !listing
            .listener
            .hears(&RoutingKey::parse("account.signed.up").expect("a declared key"))
    );
}

#[tokio::test]
async fn a_message_about_no_invitation_is_refused() {
    let listing = listing();

    assert!(
        listing
            .listener
            .handle(&told(ISSUED, &InvitationId::generate(wiring::at(1_000))))
            .await
            .is_err(),
        "a message the listener cannot act on belongs in dead letters"
    );
}

#[tokio::test]
async fn a_retired_invitation_leaves_the_list_however_late_any_message_about_it_comes() {
    let listing = listing();
    let id = listing.invited(ADA).await;
    listing
        .listener
        .handle(&told(ISSUED, &id))
        .await
        .expect("cataloguing should succeed");
    listing.wired.clock.set(
        wiring::at(1_700_000_000)
            + Duration::days(7)
            + InvitationSummary::EXPIRED_LISTED_FOR
            + Duration::SECOND,
    );
    assert_eq!(
        listing
            .invitations
            .retire_due(100)
            .await
            .expect("an invitation retires"),
        1
    );

    for routing in [RETIRED, ISSUED] {
        listing
            .listener
            .handle(&told(routing, &id))
            .await
            .expect("cataloguing should succeed");
    }
    let retired_at = listing.wired.clock.now();

    assert!(listing.listed().await.is_empty());
    assert_eq!(
        listing
            .invitations
            .forget_tombstones_laid_before(retired_at, 10)
            .await
            .expect("forgetting should succeed"),
        0,
        "the retired invitation's tombstone was laid as it was catalogued"
    );
    assert_eq!(
        listing
            .invitations
            .forget_tombstones_laid_before(retired_at + Duration::SECOND, 10)
            .await
            .expect("forgetting should succeed"),
        1,
        "the chore is told of every tombstone it forgot, or it stops before the backlog is gone"
    );
}

#[tokio::test]
async fn a_catalogued_invitation_is_found_by_the_digest_of_the_token_mailed_for_it() {
    let wired = wiring::wired();
    let issued = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite");
    wired
        .invitations
        .catalogue(None, &issued.id)
        .await
        .expect("cataloguing should succeed");

    let found = wired
        .catalog
        .with_digest(&wired.token_mailed_to(ADA).await.digest())
        .await
        .expect("finding should succeed");

    assert_eq!(found.map(|summary| summary.id), Some(issued.id));
}
