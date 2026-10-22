use std::time::{Duration, Instant};

use accounts_core::{AccountId, AccountServiceError, EmailAddress, EmailAddresses, PasswordHash};
use clock::Clock;

use crate::wiring::{ADA, HER_PASSWORD, Wired, wired};

const BEA: &str = "bea@example.com";
const HIS_PASSWORD: &str = "a horse of another colour";

async fn signed_up(wired: &Wired, email: &str, password: &str) -> AccountId {
    wired
        .accounts
        .sign_up(email, "Someone", password)
        .await
        .expect("signing up should succeed");

    wired
        .secrets
        .holder(&EmailAddress::new(email).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

async fn verified(wired: &Wired, email: &str, password: &str) -> AccountId {
    let id = signed_up(wired, email, password).await;
    wired
        .accounts
        .verify(email, &wired.last_code_sent_to(email))
        .await
        .expect("the code sent should verify");

    id
}

fn refused(signed_in: Result<AccountId, AccountServiceError>) -> bool {
    matches!(signed_in, Err(AccountServiceError::SignInRefused))
}

#[tokio::test]
async fn the_right_address_and_password_sign_a_verified_account_in() {
    let wired = wired();
    let id = verified(&wired, ADA, HER_PASSWORD).await;

    assert_eq!(
        wired
            .accounts
            .sign_in(ADA, HER_PASSWORD)
            .await
            .expect("the right pair should sign in"),
        id
    );
}

#[tokio::test]
async fn the_address_signs_in_however_it_is_typed() {
    let wired = wired();
    let id = verified(&wired, ADA, HER_PASSWORD).await;

    assert_eq!(
        wired
            .accounts
            .sign_in(" Ada@Example.COM ", HER_PASSWORD)
            .await
            .expect("the same address in other letters should sign in"),
        id
    );
}

#[tokio::test]
async fn a_wrong_password_and_an_unknown_address_are_refused_in_the_same_words() {
    let wired = wired();
    verified(&wired, ADA, HER_PASSWORD).await;

    assert!(refused(
        wired.accounts.sign_in(ADA, "not her password at all").await
    ));
    assert!(
        refused(
            wired
                .accounts
                .sign_in("nobody@example.com", HER_PASSWORD)
                .await
        ),
        "telling an unknown address apart from a wrong password says who has an account"
    );
}

#[tokio::test]
async fn an_address_and_a_password_of_two_accounts_do_not_sign_in() {
    let wired = wired();
    verified(&wired, ADA, HER_PASSWORD).await;
    verified(&wired, BEA, HIS_PASSWORD).await;

    assert!(refused(wired.accounts.sign_in(ADA, HIS_PASSWORD).await));
    assert!(refused(wired.accounts.sign_in(BEA, HER_PASSWORD).await));
}

#[tokio::test]
async fn an_address_held_but_not_yet_the_accounts_own_does_not_sign_in() {
    let wired = wired();
    let id = verified(&wired, ADA, HER_PASSWORD).await;
    let reserved = EmailAddress::new("ada@example.org").expect("a plain address");
    wired
        .secrets
        .claim(&reserved, &id)
        .await
        .expect("claiming should succeed");

    assert!(
        refused(
            wired
                .accounts
                .sign_in("ada@example.org", HER_PASSWORD)
                .await
        ),
        "an address being changed to is only a reservation until it is verified"
    );
}

#[tokio::test]
async fn an_address_claimed_for_an_account_that_never_came_to_be_does_not_sign_in() {
    let wired = wired();
    wired
        .secrets
        .claim(
            &EmailAddress::new(ADA).expect("a plain address"),
            &AccountId::generate(wired.clock.now()),
        )
        .await
        .expect("claiming should succeed");

    assert!(
        refused(wired.accounts.sign_in(ADA, HER_PASSWORD).await),
        "a server stopping between claiming an address and writing its account must not \
         leave the address failing every sign-in as the server's own fault"
    );
}

#[tokio::test]
async fn an_unverified_account_is_told_so_only_with_the_right_password() {
    let wired = wired();
    signed_up(&wired, ADA, HER_PASSWORD).await;

    assert!(matches!(
        wired.accounts.sign_in(ADA, HER_PASSWORD).await,
        Err(AccountServiceError::NotVerifiedYet)
    ));
    assert!(
        refused(wired.accounts.sign_in(ADA, "not her password at all").await),
        "without the password, an unverified account looks like no account at all"
    );
}

#[tokio::test]
async fn an_address_nobody_has_costs_the_same_work_as_a_password_checked() {
    let wired = wired();
    PasswordHash::stand_in();
    let checking = (0..3)
        .map(|_| {
            let started = Instant::now();
            let _admitted = PasswordHash::stand_in().admits(HER_PASSWORD);
            started.elapsed()
        })
        .min()
        .unwrap_or(Duration::ZERO);

    let started = Instant::now();
    let _refused = wired
        .accounts
        .sign_in("nobody@example.com", HER_PASSWORD)
        .await;
    let signing_in = started.elapsed();

    assert!(
        signing_in >= checking / 4,
        "an unknown address answered in {signing_in:?} where a password takes {checking:?} \
         tells by its speed that nobody has it"
    );
}
