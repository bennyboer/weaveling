use accounts_core::{
    AccountId, EmailAddress, EmailAddresses, InvitationTicket, InvitationTickets, InvitationToken,
};
use clock::Clock;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

async fn abandoned(wired: &Wired) -> AccountId {
    let never_begun = AccountId::generate(wired.clock.now());
    wired
        .secrets
        .claim(&address(), &never_begun, wired.clock.now())
        .await
        .expect("claiming should succeed");

    never_begun
}

fn address() -> EmailAddress {
    EmailAddress::new(ADA).expect("a plain address")
}

async fn holder(wired: &Wired) -> Option<AccountId> {
    wired
        .secrets
        .holder(&address())
        .await
        .expect("reading should succeed")
}

fn later(wired: &Wired, by: Duration) {
    wired.clock.set(wired.clock.now() + by);
}

#[tokio::test]
async fn an_address_left_claimed_by_a_sign_up_that_never_came_to_be_is_free_again() {
    let wired = wiring::wired();
    let never_begun = abandoned(&wired).await;
    later(&wired, Duration::minutes(6));

    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    assert_ne!(holder(&wired).await, Some(never_begun));
    assert_eq!(
        wired.codes_sent_to(ADA).len(),
        1,
        "a server stopping between claiming an address and writing its account must not keep the address from anyone for good"
    );
}

#[tokio::test]
async fn an_address_claimed_moments_ago_is_left_to_the_sign_up_under_way() {
    let wired = wiring::wired();
    let under_way = abandoned(&wired).await;
    later(&wired, Duration::minutes(4));

    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    assert_eq!(holder(&wired).await, Some(under_way));
    assert!(wired.codes_sent_to(ADA).is_empty());
}

#[tokio::test]
async fn an_address_held_by_an_account_is_never_taken_over_however_old() {
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let first = holder(&wired).await;
    later(&wired, Duration::days(30));

    wired
        .accounts
        .sign_up(ADA, "Mallory", "another long password")
        .await
        .expect("signing up should succeed");

    assert_eq!(holder(&wired).await, first);
}

#[tokio::test]
async fn an_invitation_to_an_address_left_claimed_still_signs_it_up() {
    let wired = wiring::wired();
    let never_begun = abandoned(&wired).await;
    wired
        .secrets
        .keep_ticket(
            &InvitationToken::presented("a token").digest(),
            &InvitationTicket {
                invitation: "invitation_1".to_owned(),
                to: address(),
                until: wired.clock.now() + Duration::days(7),
            },
        )
        .await
        .expect("keeping should succeed");
    later(&wired, Duration::minutes(6));

    wired
        .accounts
        .sign_up_by_invitation("a token", "Ada", HER_PASSWORD)
        .await
        .expect("an abandoned claim is no account");

    assert_ne!(holder(&wired).await, Some(never_begun));
}

#[tokio::test]
async fn asking_for_a_code_again_for_an_address_left_claimed_sends_nothing() {
    let wired = wiring::wired();
    abandoned(&wired).await;

    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("an address without an account is answered like any other");

    assert!(wired.mailbox.sent_to(ADA).is_empty());
}
