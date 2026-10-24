use eventsourcing::{AggregateId, EventStore, ServiceError, Version};
use invitations_core::{InvitationEvent, InvitationId, InvitationServiceError, KIND};
use time::Duration;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

async fn told(wired: &Wired, id: &InvitationId) -> Vec<InvitationEvent> {
    wired
        .store
        .read_from(&AggregateId::from(id), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| recorded.event)
        .collect()
}

#[tokio::test]
async fn an_operator_invites_an_address_and_it_is_mailed_a_link_to_the_instance() {
    let wired = wiring::wired();

    let id = wired
        .invitations
        .issue(&wiring::an_operator(), " Ada@Example.com ", ORIGIN)
        .await
        .expect("an operator may invite")
        .id;

    let links = wired.links_mailed_to(ADA);
    assert_eq!(links.len(), 1);
    assert!(
        links[0].starts_with("https://weaveling.example.com/invitation#"),
        "{}",
        links[0]
    );
    let token = wired.token_mailed_to(ADA);
    assert!(
        matches!(
            told(&wired, &id).await.as_slice(),
            [InvitationEvent::Issued { to, digest, until }]
                if to.as_str() == ADA
                    && *digest == token.digest()
                    && *until == wiring::at(1_700_000_000) + Duration::days(7)
        ),
        "the invitation keeps its link's token by its digest alone, and holds for seven days"
    );
}

#[tokio::test]
async fn the_link_is_made_from_the_origin_however_it_ends() {
    let wired = wiring::wired();

    wired
        .invitations
        .issue(
            &wiring::an_operator(),
            ADA,
            "https://weaveling.example.com/",
        )
        .await
        .expect("an operator may invite");

    assert!(
        wired.links_mailed_to(ADA)[0].starts_with("https://weaveling.example.com/invitation#"),
        "{:?}",
        wired.links_mailed_to(ADA)
    );
}

#[tokio::test]
async fn only_an_operator_invites_or_withdraws() {
    let wired = wiring::wired();

    assert!(matches!(
        wired
            .invitations
            .issue(&wiring::no_operator(), ADA, ORIGIN)
            .await,
        Err(InvitationServiceError::NotAnOperator)
    ));
    assert!(wired.mailbox.sent_to(ADA).is_empty());

    let id = wired
        .invitations
        .issue(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id;
    assert!(matches!(
        wired
            .invitations
            .withdraw(&wiring::no_operator(), &id)
            .await,
        Err(InvitationServiceError::NotAnOperator)
    ));
    assert_eq!(told(&wired, &id).await.len(), 1);
}

#[tokio::test]
async fn an_address_that_could_never_be_mailed_is_refused() {
    let wired = wiring::wired();

    assert!(matches!(
        wired
            .invitations
            .issue(&wiring::an_operator(), "ada", ORIGIN)
            .await,
        Err(InvitationServiceError::InvalidEmail(_))
    ));
}

#[tokio::test]
async fn an_address_invited_three_times_must_wait_before_the_fourth_while_another_need_not() {
    let wired = wiring::wired();
    for _ in 0..3 {
        wired
            .invitations
            .issue(&wiring::an_operator(), ADA, ORIGIN)
            .await
            .expect("three invitations to one address are free");
    }

    assert!(matches!(
        wired.invitations.issue(&wiring::an_operator(), ADA, ORIGIN).await,
        Err(InvitationServiceError::Throttled { wait }) if wait == Duration::minutes(1)
    ));
    assert_eq!(
        wired.mailbox.sent_to(ADA).len(),
        3,
        "or an operator's account taken over could fill a stranger's inbox"
    );
    wired
        .invitations
        .issue(&wiring::an_operator(), "grace@example.com", ORIGIN)
        .await
        .expect("another address is counted apart");

    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(1));
    for _ in 0..3 {
        wired
            .invitations
            .issue(&wiring::an_operator(), ADA, ORIGIN)
            .await
            .expect("a day later the count starts over, three free again");
    }
}

#[tokio::test]
async fn an_operator_withdraws_an_invitation() {
    let wired = wiring::wired();
    let id = wired
        .invitations
        .issue(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id;

    wired
        .invitations
        .withdraw(&wiring::an_operator(), &id)
        .await
        .expect("an operator may withdraw");

    assert_eq!(
        told(&wired, &id).await.last(),
        Some(&InvitationEvent::Withdrawn)
    );
}

#[tokio::test]
async fn an_invitation_that_is_not_there_is_not_withdrawn() {
    let wired = wiring::wired();

    assert!(matches!(
        wired
            .invitations
            .withdraw(
                &wiring::an_operator(),
                &InvitationId::generate(wiring::at(1_000))
            )
            .await,
        Err(InvitationServiceError::Events(
            ServiceError::NotFound { .. }
        ))
    ));
}
