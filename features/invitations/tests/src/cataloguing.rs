use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use inbox::Inboxed;
use invitations_catalog::InMemoryInvitationCatalog;
use invitations_contract::{ACCEPTED, DECLINED, ISSUED, RETIRED, WITHDRAWN};
use invitations_core::{
    DeclineReason, InvitationCatalog, InvitationEvent, InvitationId, InvitationSummary,
    InvitationsCore, KIND, Outcome,
};
use invitations_messaging::CatalogueInvitationsListener;
use messaging::{Listener, Message, RoutingKey};
use outbox::Outbox;
use serde_json::json;
use throttling::InMemoryThrottles;
use time::Duration;
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

fn a_message(routing: &str, invitation: &InvitationId) -> Message {
    eventpublishing::published(
        routing,
        "invitation",
        &invitation.to_string(),
        1,
        json!({ "version": 0, "name": "WHATEVER" }),
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

struct Listing {
    wired: Wired,
    catalog: Arc<InMemoryInvitationCatalog>,
    invitations: InvitationsCore,
    listener: Inboxed<CatalogueInvitationsListener>,
}

fn listing() -> Listing {
    let wired = wiring::a_workbench();
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
    async fn an_open_invitation_to(&self, to: &str) -> InvitationId {
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
    // given: an issued invitation
    let listing = listing();
    let id = listing.an_open_invitation_to(ADA).await;

    // when: its issue is heard
    listing
        .listener
        .handle(&a_message(ISSUED, &id))
        .await
        .expect("cataloguing should succeed");

    // then: it is listed as it stands
    let listed = listing.listed().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].to.as_str(), ADA);
    assert_eq!(listed[0].issued_at, clock::at(1_700_000_000));
    assert_eq!(
        listed[0].until,
        clock::at(1_700_000_000) + Duration::days(7)
    );
    assert_eq!(listed[0].outcome, Outcome::Open);
}

#[tokio::test]
async fn an_issue_heard_after_its_invitation_was_taken_up_lists_nothing() {
    // given: an invitation taken up
    let listing = listing();
    let id = listing.an_open_invitation_to(ADA).await;
    listing
        .wired
        .invitations
        .taken_up(None, &id, &Agent::Account("account_ada".into()))
        .await
        .expect("an open invitation is accepted");

    // when: its issue is heard
    listing
        .listener
        .handle(&a_message(ISSUED, &id))
        .await
        .expect("cataloguing should succeed");

    // then: nothing is listed
    assert!(
        listing.listed().await.is_empty(),
        "the list reads how the invitation stands now, not what the message was about"
    );
}

#[tokio::test]
async fn a_declined_invitation_stays_listed_and_says_why() {
    // given: a declined invitation
    let listing = listing();
    let id = listing.an_open_invitation_to(ADA).await;
    listing
        .wired
        .invitations
        .declined(None, &id, DeclineReason::AddressHasAccount)
        .await
        .expect("an open invitation is declined");

    // when: its declining is heard
    listing
        .listener
        .handle(&a_message(DECLINED, &id))
        .await
        .expect("cataloguing should succeed");

    // then: it is listed with why it was declined
    assert_eq!(
        listing.listed().await[0].outcome,
        Outcome::Declined(DeclineReason::AddressHasAccount)
    );
}

#[tokio::test]
async fn every_change_to_an_invitation_is_heard() {
    // given: the listener cataloguing invitations
    let listing = listing();

    // then: it hears every change to an invitation
    for routing in [ISSUED, ACCEPTED, WITHDRAWN, DECLINED] {
        assert!(
            listing
                .listener
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    // and: nothing else
    assert!(
        !listing
            .listener
            .hears(&RoutingKey::parse("account.signed.up").expect("a declared key"))
    );
}

#[tokio::test]
async fn a_message_about_no_invitation_is_refused() {
    // given: the listener cataloguing invitations
    let listing = listing();

    // then: a message about no invitation is refused
    assert!(
        listing
            .listener
            .handle(&a_message(
                ISSUED,
                &InvitationId::generate(clock::at(1_000))
            ))
            .await
            .is_err(),
        "a message the listener cannot act on belongs in dead letters"
    );
}

#[tokio::test]
async fn a_retired_invitation_is_collapsed_to_a_snapshot_that_keeps_no_address() {
    // given: an invitation to ada, listed long enough to retire
    let listing = listing();
    let id = listing.an_open_invitation_to(ADA).await;
    listing
        .listener
        .handle(&a_message(ISSUED, &id))
        .await
        .expect("cataloguing should succeed");
    listing.wired.clock.set(
        clock::at(1_700_000_000)
            + Duration::days(7)
            + InvitationSummary::EXPIRED_LISTED_FOR
            + Duration::SECOND,
    );

    // and: its issue published
    listing
        .wired
        .events
        .deliver(100)
        .await
        .expect("an outbox in memory always delivers");
    assert_eq!(listing.wired.events.kept_published(), ["invitation.issued"]);

    // when: it retires
    listing
        .invitations
        .retire_due(100)
        .await
        .expect("an invitation retires");

    // then: its stream is one snapshot, retired
    let stream = listing
        .wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(stream.len(), 1, "every event before the snapshot is gone");
    assert!(matches!(
        &stream[0].event,
        InvitationEvent::Snapshotted { retired: true, .. }
    ));
    // and: nothing in it holds her address
    assert!(
        !format!("{:?}", stream[0].event).contains(ADA),
        "the address goes with the retiring"
    );
    // and: nothing it told is kept, its retiring waiting to be told
    assert!(
        listing.wired.events.kept_published().is_empty(),
        "the issue named her address"
    );
    assert_eq!(listing.wired.events.waiting_to_be_published(), 1);
}

#[tokio::test]
async fn a_retired_invitation_leaves_the_list_however_late_any_message_about_it_comes() {
    // given: a catalogued invitation
    let listing = listing();
    let id = listing.an_open_invitation_to(ADA).await;
    listing
        .listener
        .handle(&a_message(ISSUED, &id))
        .await
        .expect("cataloguing should succeed");
    // and: it retired once listed long enough
    listing.wired.clock.set(
        clock::at(1_700_000_000)
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

    // when: its retiring and then its issue are heard
    for routing in [RETIRED, ISSUED] {
        listing
            .listener
            .handle(&a_message(routing, &id))
            .await
            .expect("cataloguing should succeed");
    }
    // then: it is listed no more
    let retired_at = listing.wired.clock.now();

    assert!(listing.listed().await.is_empty());
    // and: its tombstone is laid as it is catalogued
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
    // given: a catalogued invitation
    let wired = wiring::a_workbench();
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

    // when: it is looked up by the digest of the token mailed for it
    let found = wired
        .catalog
        .with_digest(&wired.token_mailed_to(ADA).await.digest())
        .await
        .expect("finding should succeed");

    // then: it is found
    assert_eq!(found.map(|summary| summary.id), Some(issued.id));
}
