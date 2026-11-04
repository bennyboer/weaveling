use accounts_core::{
    AccountError, AccountEvent, AccountId, AccountsError, EmailAddress, EmailAddresses, KIND,
};
use eventsourcing::{AggregateId, EventStore, ServiceError, StoreError, Version};

use crate::session_ends;
use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";

async fn a_verified_account(wired: &Wired, email: &str) -> AccountId {
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
    let ada = a_verified_account(wired, ADA).await;
    let grace = a_verified_account(wired, GRACE).await;
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
    // given: an account and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    // and: a session of the account
    let (token, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in before she is locked");

    // when: the operator locks the account
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    // then: it is locked
    assert!(is_locked(&wired, &ada).await);
    // and: its session is over
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
    // given: an account and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;

    // when: the operator locks the account
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    // then: the lock is told as done by the operator
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
    // given: a locked account
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    // then: signing in with the right password is refused as locked
    assert!(matches!(
        wired.accounts.sign_in(ADA, HER_PASSWORD).await,
        Err(AccountsError::Locked)
    ));
    // and: with a wrong one is refused as any wrong password is
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
    // given: two accounts and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    let hedy = a_verified_account(&wired, "hedy@example.com").await;

    // then: an account that is no operator may not lock another
    assert!(matches!(
        wired.accounts.lock(&ada, &hedy, "I do not like her.").await,
        Err(AccountsError::NotAnOperator)
    ));
    // and: the other stays unlocked
    assert!(!is_locked(&wired, &hedy).await);

    // when: the operator locks it
    wired
        .accounts
        .lock(&grace, &hedy, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    // then: the account that is no operator may not unlock it
    assert!(matches!(
        wired.accounts.unlock(&ada, &hedy).await,
        Err(AccountsError::NotAnOperator)
    ));
    // and: it stays locked
    assert!(is_locked(&wired, &hedy).await);
}

#[tokio::test]
async fn an_unlocked_account_signs_in_again() {
    // given: a locked account
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    // when: the operator unlocks it
    wired
        .accounts
        .unlock(&grace, &ada)
        .await
        .expect("an operator may unlock an account");

    // then: it signs in
    wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("unlocked, she signs in again");
}

#[tokio::test]
async fn an_operator_cannot_lock_themselves() {
    // given: an operator
    let wired = wiring::wired();
    let (_ada, grace) = ada_and_grace_the_operator(&wired).await;

    // then: locking itself is refused
    assert!(matches!(
        wired.accounts.lock(&grace, &grace, "Testing.").await,
        Err(AccountsError::Events(ServiceError::Refused(
            AccountError::OperatorCannotBeLocked
        )))
    ));
}

#[tokio::test]
async fn an_account_is_locked_only_with_a_reason() {
    // given: an account and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;

    // then: a lock with a blank reason is refused
    assert!(matches!(
        wired.accounts.lock(&grace, &ada, "   ").await,
        Err(AccountsError::InvalidLockReason(_))
    ));
    // and: the account stays unlocked
    assert!(!is_locked(&wired, &ada).await);
}

#[tokio::test]
async fn a_locked_account_may_set_a_new_password_but_stays_locked() {
    // given: a locked account
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    // when: it resets its password
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

    // then: signing in with the new password is refused as locked
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
    // given: an account and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    // and: a session of the account ended by a lock
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

    // when: the operator unlocks the account later
    wired.clock.set(wiring::at(1_700_000_120));
    wired
        .accounts
        .unlock(&grace, &ada)
        .await
        .expect("an operator may unlock an account");

    // then: the session stays over
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

#[tokio::test]
async fn a_lock_reaches_the_end_of_the_account_s_sessions_at_once() {
    // given: an account and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;
    // and: someone waiting on the end of the account's session
    let (token, signed_in) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in before she is locked");
    let waiting =
        session_ends::awaited(wired.accounts.session_end(&token, signed_in.session.until)).await;

    // when: the operator locks the account
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    // then: the end is reached at once
    assert!(session_ends::reached(waiting).await);
}

#[tokio::test]
async fn a_lock_that_lost_a_race_is_tried_again_and_lands() {
    // given: an account and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;

    // when: the operator locks the account, losing four races first
    wired
        .losing_races(4)
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("the fifth try lands");

    // then: it is locked
    assert!(is_locked(&wired, &ada).await);
}

#[tokio::test]
async fn a_lock_that_lost_every_race_it_may_try_is_a_conflict() {
    // given: an account and an operator
    let wired = wiring::wired();
    let (ada, grace) = ada_and_grace_the_operator(&wired).await;

    // when: the operator locks the account, losing every race it may try
    let refused = wired.losing_races(5).lock(&grace, &ada, "Sent spam.").await;

    // then: it is refused as outdated
    assert!(matches!(
        refused,
        Err(AccountsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: the account stays unlocked
    assert!(!is_locked(&wired, &ada).await);
}
