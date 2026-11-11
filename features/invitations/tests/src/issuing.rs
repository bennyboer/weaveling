use acting::Acting;
use eventsourcing::{Agent, AggregateId, EventStore, ServiceError, Version};
use invitations_core::{InvitationEvent, InvitationId, InvitationsError, KIND, Scope};
use time::Duration;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

async fn events_of(wired: &Wired, id: &InvitationId) -> Vec<InvitationEvent> {
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
    // given: an operator
    let wired = wiring::a_workbench();

    // when: it invites an address however written
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), " Ada@Example.com ", ORIGIN)
        .await
        .expect("an operator may invite")
        .id;

    // then: one link to the instance is mailed to it
    let links = wired.links_mailed_to(ADA).await;
    assert_eq!(links.len(), 1);
    assert!(
        links[0].starts_with("https://weaveling.example.com/invitation#"),
        "{}",
        links[0]
    );
    // and: the invitation keeps the token by its digest alone and holds for seven days
    let token = wired.token_mailed_to(ADA).await;
    assert!(
        matches!(
            events_of(&wired, &id).await.as_slice(),
            [InvitationEvent::Issued { to, digest, until, scope, by_an_operator, origin }]
                if to.as_str() == ADA
                    && *digest == token.digest()
                    && *until == clock::at(1_700_000_000) + Duration::days(7)
                    && *scope == Scope::Instance
                    && *by_an_operator
                    && origin == ORIGIN
        ),
        "the invitation keeps its link's token by its digest alone, and holds for seven days"
    );
}

#[tokio::test]
async fn the_link_is_made_from_the_origin_however_it_ends() {
    // given: an operator
    let wired = wiring::a_workbench();

    // when: it invites from an origin ending in a slash
    wired
        .invitations
        .invite_to_instance(
            &wiring::an_operator(),
            ADA,
            "https://weaveling.example.com/",
        )
        .await
        .expect("an operator may invite");

    // then: the link is made from the origin all the same
    assert!(
        wired.links_mailed_to(ADA).await[0]
            .starts_with("https://weaveling.example.com/invitation#"),
        "{:?}",
        wired.links_mailed_to(ADA).await
    );
}

#[tokio::test]
async fn only_an_operator_invites_or_withdraws() {
    // given: someone who is no operator
    let wired = wiring::a_workbench();

    // then: their inviting is refused
    assert!(matches!(
        wired
            .invitations
            .invite_to_instance(&wiring::no_operator(), ADA, ORIGIN)
            .await,
        Err(InvitationsError::NotAnOperator)
    ));
    // and: nothing is mailed
    assert!(wired.sent_to(ADA).await.is_empty());

    // when: an operator invites
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id;
    // then: their withdrawing is refused
    assert!(matches!(
        wired
            .invitations
            .withdraw(&wiring::no_operator(), &id, None)
            .await,
        Err(InvitationsError::NotAnOperator)
    ));
    // and: the invitation stays open
    assert_eq!(events_of(&wired, &id).await.len(), 1);
}

#[tokio::test]
async fn an_address_that_could_never_be_mailed_is_refused() {
    // given: an operator
    let wired = wiring::a_workbench();

    // then: an address that could never be mailed is refused
    assert!(matches!(
        wired
            .invitations
            .invite_to_instance(&wiring::an_operator(), "ada", ORIGIN)
            .await,
        Err(InvitationsError::InvalidEmail(_))
    ));
}

#[tokio::test]
async fn an_address_invited_three_times_must_wait_before_the_fourth_while_another_need_not() {
    // given: an address invited three times
    let wired = wiring::a_workbench();
    for _ in 0..3 {
        wired
            .invitations
            .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
            .await
            .expect("three invitations to one address are free");
    }

    // then: a fourth time is refused for a minute
    assert!(matches!(
        wired.invitations.invite_to_instance(&wiring::an_operator(), ADA, ORIGIN).await,
        Err(InvitationsError::Throttled { wait }) if wait == Duration::minutes(1)
    ));
    // and: nothing more is mailed
    assert_eq!(
        wired.sent_to(ADA).await.len(),
        3,
        "or an operator's account taken over could fill a stranger's inbox"
    );
    // and: another address is still invited
    wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), "grace@example.com", ORIGIN)
        .await
        .expect("another address is counted apart");

    // when: a day passes
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(1));
    // then: the address is invited three times more
    for _ in 0..3 {
        wired
            .invitations
            .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
            .await
            .expect("a day later the count starts over, three free again");
    }
}

#[tokio::test]
async fn an_operator_withdraws_an_invitation() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id;

    // when: an operator withdraws it
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &id, None)
        .await
        .expect("an operator may withdraw");

    // then: it is withdrawn
    assert_eq!(
        events_of(&wired, &id).await.last(),
        Some(&InvitationEvent::Withdrawn)
    );
}

#[tokio::test]
async fn an_invitation_that_is_not_there_is_not_withdrawn() {
    // given: an operator
    let wired = wiring::a_workbench();

    // then: withdrawing an invitation that is not there is refused as not found
    assert!(matches!(
        wired
            .invitations
            .withdraw(
                &wiring::an_operator(),
                &InvitationId::generate(clock::at(1_000)),
                None
            )
            .await,
        Err(InvitationsError::Events(ServiceError::NotFound { .. }))
    ));
}

#[tokio::test]
async fn an_account_invites_so_many_each_hour_while_another_still_may() {
    // given: an instance letting each account invite two an hour, at twenty to the hour and a little
    let wired = wiring::a_workbench();
    let invitations = wired.invitations.clone().capping_invitations(2);
    let invited = |by: Acting, address: &'static str| {
        let invitations = invitations.clone();
        async move { invitations.invite_to_instance(&by, address, ORIGIN).await }
    };

    // when: one operator invites three addresses, each once
    invited(wiring::an_operator(), "one@example.com")
        .await
        .expect("the first is mailed");
    invited(wiring::an_operator(), "two@example.com")
        .await
        .expect("the second is mailed");
    let third = invited(wiring::an_operator(), "three@example.com").await;

    // then: the third waits for the next hour, though no address was invited twice
    assert!(
        matches!(
            third,
            Err(InvitationsError::Throttled { wait })
                if wait == Duration::minutes(46) + Duration::seconds(40)
        ),
        "a cap by address alone lets an account taken over mail any number of strangers: {third:?}"
    );
    assert!(wired.sent_to("three@example.com").await.is_empty());
    // and: another operator still invites
    invited(
        Acting {
            agent: Agent::Account("account_bea".into()),
            operator: true,
        },
        "three@example.com",
    )
    .await
    .expect("each account is counted on its own");
}
