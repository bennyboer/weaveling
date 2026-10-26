use accounts_core::{
    AccountError, AccountEvent, AccountId, AccountsError, EmailAddress, EmailAddresses, KIND,
};
use eventsourcing::{AggregateId, EventStore, ServiceError, Version};

use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";

async fn signed_up(wired: &Wired, email: &str) -> AccountId {
    wired
        .accounts
        .sign_up(email, "Someone", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(email, &wired.last_code_sent_to(email))
        .await
        .expect("the code sent should verify");

    wired
        .secrets
        .holder(&EmailAddress::new(email).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

async fn ada_and_grace_the_operator(wired: &Wired) -> (AccountId, AccountId) {
    let ada = signed_up(wired, ADA).await;
    let grace = signed_up(wired, GRACE).await;
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    wired
        .accounts
        .become_operator(&grace, code.as_str())
        .await
        .expect("the code drawn makes an operator");

    (ada, grace)
}

async fn is_locked(wired: &Wired, id: &AccountId) -> bool {
    wired
        .account(id)
        .await
        .expect("the account should exist")
        .state
        .is_locked()
}

#[tokio::test]
async fn an_operator_locks_an_account_and_its_sessions_end_at_once() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    let (token, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in before she is locked");

    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    assert!(is_locked(&wired, &ada).await);
    assert_eq!(
        wired
            .accounts
            .signed_in(&token)
            .await
            .expect("asking should succeed"),
        None
    );
}

#[tokio::test]
async fn a_lock_is_told_as_the_operators_doing() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;

    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    let told = wired
        .store
        .read_from(&AggregateId::from(&ada), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    let locked = told
        .iter()
        .find(|recorded| matches!(recorded.event, AccountEvent::Locked { .. }))
        .expect("the lock should be told");
    assert_eq!(locked.metadata.agent, grace.acting());
}

#[tokio::test]
async fn a_locked_account_is_refused_at_signing_in_only_once_its_password_is_right() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    assert!(matches!(
        wired.accounts.sign_in(ADA, HER_PASSWORD).await,
        Err(AccountsError::Locked)
    ));
    assert!(
        matches!(
            wired.accounts.sign_in(ADA, "not her password at all").await,
            Err(AccountsError::SignInRefused)
        ),
        "whoever does not know the password learns nothing of the lock"
    );
}

#[tokio::test]
async fn only_an_operator_locks_or_unlocks() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    let hedy = signed_up(&wired, "hedy@example.com").await;

    assert!(matches!(
        wired.accounts.lock(&ada, &hedy, "I do not like her.").await,
        Err(AccountsError::NotAnOperator)
    ));
    assert!(!is_locked(&wired, &hedy).await);

    wired
        .accounts
        .lock(&grace, &hedy, "Sent spam.")
        .await
        .expect("an operator may lock an account");
    assert!(matches!(
        wired.accounts.unlock(&ada, &hedy).await,
        Err(AccountsError::NotAnOperator)
    ));
    assert!(is_locked(&wired, &hedy).await);
}

#[tokio::test]
async fn an_unlocked_account_signs_in_again() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    wired
        .accounts
        .unlock(&grace, &ada)
        .await
        .expect("an operator may unlock an account");

    wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("unlocked, she signs in again");
}

#[tokio::test]
async fn an_operator_cannot_lock_themselves() {
    let wired = wiring::wired();
    let (_ada, grace) = ada_and_grace_the_operator(&wired).await;

    assert!(matches!(
        wired.accounts.lock(&grace, &grace, "Testing.").await,
        Err(AccountsError::Events(ServiceError::Refused(
            AccountError::OperatorCannotBeLocked
        )))
    ));
}

#[tokio::test]
async fn an_account_is_locked_only_with_a_reason() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;

    assert!(matches!(
        wired.accounts.lock(&grace, &ada, "   ").await,
        Err(AccountsError::InvalidLockReason(_))
    ));
    assert!(!is_locked(&wired, &ada).await);
}

#[tokio::test]
async fn a_locked_account_may_set_a_new_password_but_stays_locked() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    wired
        .accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("asking is answered alike for everyone");
    wired
        .accounts
        .reset_password(ADA, &wired.last_code_sent_to(ADA), "a new password at last")
        .await
        .expect("refusing the reset would tell that she is locked");

    assert!(
        matches!(
            wired.accounts.sign_in(ADA, "a new password at last").await,
            Err(AccountsError::Locked)
        ),
        "a reset never unlocks"
    );
}

#[tokio::test]
async fn unlocking_brings_back_no_session_the_lock_ended() {
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    let (token, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in before she is locked");
    wired.clock.set(wiring::at(1_700_000_060));
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    wired.clock.set(wiring::at(1_700_000_120));
    wired
        .accounts
        .unlock(&grace, &ada)
        .await
        .expect("an operator may unlock an account");

    assert_eq!(
        wired
            .accounts
            .signed_in(&token)
            .await
            .expect("asking should succeed"),
        None,
        "she signs in anew"
    );
}
