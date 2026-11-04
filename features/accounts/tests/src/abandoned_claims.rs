use accounts_core::{
    AccountId, EmailAddress, EmailAddresses, InvitationScope, InvitationTicket, InvitationToken,
};
use clock::Clock;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

async fn a_claim_with_no_account(wired: &Wired) -> AccountId {
    let never_begun = AccountId::generate(wired.clock.now());
    wired
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
    // given: an address claimed by no account
    let wired = wiring::wired();
    let never_begun = a_claim_with_no_account(&wired).await;
    // and: a while gone by since
    later(&wired, Duration::minutes(6));

    // when: someone signs up with it
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    // then: the address is theirs
    assert_ne!(holder(&wired).await, Some(never_begun));
    // and: a code is sent to it
    assert_eq!(
        wired.codes_sent_to(ADA).len(),
        1,
        "a server stopping between claiming an address and writing its account must not keep the address from anyone for good"
    );
}

#[tokio::test]
async fn an_address_claimed_moments_ago_is_left_to_the_sign_up_under_way() {
    // given: an address claimed by no account
    let wired = wiring::wired();
    let under_way = a_claim_with_no_account(&wired).await;
    // and: only moments gone by since
    later(&wired, Duration::minutes(4));

    // when: someone signs up with it
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    // then: the claim stands
    assert_eq!(holder(&wired).await, Some(under_way));
    // and: no code is sent
    assert!(wired.codes_sent_to(ADA).is_empty());
}

#[tokio::test]
async fn an_address_held_by_an_account_is_never_taken_over_however_old() {
    // given: an account holding an address
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let first = holder(&wired).await;
    // and: a long while gone by since
    later(&wired, Duration::days(30));

    // when: someone else signs up with it
    wired
        .accounts
        .sign_up(ADA, "Mallory", "another long password")
        .await
        .expect("signing up should succeed");

    // then: the account still holds it
    assert_eq!(holder(&wired).await, first);
}

#[tokio::test]
async fn an_invitation_to_an_address_left_claimed_still_signs_it_up() {
    // given: an address claimed by no account
    let wired = wiring::wired();
    let never_begun = a_claim_with_no_account(&wired).await;
    // and: an invitation to it
    wired
        .keep_ticket(
            &InvitationToken::presented("a token").digest(),
            &InvitationTicket {
                invitation: "invitation_1".to_owned(),
                to: address(),
                until: wired.clock.now() + Duration::days(7),
                scope: InvitationScope::Instance,
                by_an_operator: true,
            },
        )
        .await
        .expect("keeping should succeed");
    // and: a while gone by since the claim
    later(&wired, Duration::minutes(6));

    // when: the invitation is used to sign up
    wired
        .accounts
        .sign_up_by_invitation("a token", "Ada", HER_PASSWORD)
        .await
        .expect("an abandoned claim is no account");

    // then: the address is the new account's
    assert_ne!(holder(&wired).await, Some(never_begun));
}

#[tokio::test]
async fn asking_for_a_code_again_for_an_address_left_claimed_sends_nothing() {
    // given: an address claimed by no account
    let wired = wiring::wired();
    a_claim_with_no_account(&wired).await;

    // when: a code is asked for again
    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("an address without an account is answered like any other");

    // then: nothing is sent
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}
