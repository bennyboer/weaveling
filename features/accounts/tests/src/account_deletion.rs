use accounts_core::{
    AccountError, AccountId, AccountsError, DeletionAskedBy, EmailAddress, EmailAddresses,
    KEPT_BEFORE_DELETION, PendingDeletion,
};
use eventsourcing::ServiceError;

use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";
const SIGNED_UP_AT: i64 = 1_700_000_000;
const A_MINUTE_LATER: i64 = SIGNED_UP_AT + 60;

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

async fn the_operator(wired: &Wired, email: &str) -> AccountId {
    let operator = a_verified_account(wired, email).await;
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    wired
        .accounts
        .become_operator(&operator, code.as_str())
        .await
        .expect("the code drawn makes an operator");

    operator
}

async fn another_operator(wired: &Wired, by: &AccountId, email: &str) -> AccountId {
    let operator = a_verified_account(wired, email).await;
    wired
        .accounts
        .appoint_operator(by, &operator, None)
        .await
        .expect("an operator appoints another");

    operator
}

async fn pending(wired: &Wired, id: &AccountId) -> Option<PendingDeletion> {
    wired
        .account(id)
        .await
        .expect("the account should exist")
        .state
        .pending_deletion()
}

async fn is_operator(wired: &Wired, id: &AccountId) -> bool {
    wired
        .account(id)
        .await
        .expect("the account should exist")
        .state
        .is_operator()
}

#[tokio::test]
async fn the_holder_asks_for_their_account_to_be_deleted_for_its_password_and_is_signed_out() {
    // given: an account signed in
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let (token, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in");

    // when: she asks for her account to be deleted with a wrong password
    let refused = wired.accounts.ask_deletion(&ada, "not her password").await;

    // then: it is refused, and nothing is asked
    assert!(matches!(refused, Err(AccountsError::PasswordWrong)));
    assert_eq!(pending(&wired, &ada).await, None);
    // when: she asks with her password, a minute later
    wired.clock.set(clock::at(A_MINUTE_LATER));
    let until = wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");
    // then: it is kept thirty days from now, asked by her
    assert_eq!(until, clock::at(A_MINUTE_LATER) + KEPT_BEFORE_DELETION);
    assert_eq!(
        pending(&wired, &ada).await,
        Some(PendingDeletion {
            until,
            asked_by: DeletionAskedBy::Holder,
        })
    );
    // and: her session is over
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
async fn an_operator_deleting_their_account_stops_being_one_unless_the_last() {
    // given: the only operator
    let wired = wiring::wired();
    let grace = the_operator(&wired, GRACE).await;

    // when: she asks for her account to be deleted
    let refused = wired.accounts.ask_deletion(&grace, HER_PASSWORD).await;

    // then: it is refused, as the last operator, and she stays one
    assert!(matches!(refused, Err(AccountsError::LastOperator)));
    assert_eq!(pending(&wired, &grace).await, None);
    assert!(is_operator(&wired, &grace).await);
    // given: another operator
    let ada = another_operator(&wired, &grace, ADA).await;
    // when: she asks again
    wired
        .accounts
        .ask_deletion(&grace, HER_PASSWORD)
        .await
        .expect("no longer the last operator, she may");
    // then: she is no operator any more, her account to be deleted
    assert!(!is_operator(&wired, &grace).await);
    assert!(pending(&wired, &grace).await.is_some());
    // and: the other is the last operator now, and may not go too
    assert!(is_operator(&wired, &ada).await);
    assert!(matches!(
        wired.accounts.ask_deletion(&ada, HER_PASSWORD).await,
        Err(AccountsError::LastOperator)
    ));
}

#[tokio::test]
async fn the_operator_marks_an_account_for_deletion_and_only_the_operator_calls_it_off() {
    // given: an account and an operator
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let grace = the_operator(&wired, GRACE).await;

    // when: the operator marks the account for deletion
    let until = wired
        .accounts
        .mark_for_deletion(&grace, &ada, None)
        .await
        .expect("an operator marks an account");

    // then: it is to be deleted, as the operator asked
    assert_eq!(
        pending(&wired, &ada).await,
        Some(PendingDeletion {
            until,
            asked_by: DeletionAskedBy::Operator,
        })
    );
    // when: the holder tries to keep it
    let refused = wired.accounts.keep_account(&ada).await;
    // then: it is refused, as only the operator calls it off
    assert!(matches!(
        refused,
        Err(AccountsError::Events(ServiceError::Refused(
            AccountError::DeletionAskedByTheOperator
        )))
    ));
    assert!(pending(&wired, &ada).await.is_some());
    // when: the operator keeps it
    wired
        .accounts
        .keep_account_of(&grace, &ada, None)
        .await
        .expect("the operator calls it off");
    // then: it is kept, and signs in again
    assert_eq!(pending(&wired, &ada).await, None);
    wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("a kept account signs in");
}

#[tokio::test]
async fn the_holder_keeps_the_account_they_asked_to_delete() {
    // given: an account its holder asked to delete
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");

    // when: she keeps it
    wired
        .accounts
        .keep_account(&ada)
        .await
        .expect("she calls off what she asked");

    // then: it is no longer to be deleted
    assert_eq!(pending(&wired, &ada).await, None);
}

#[tokio::test]
async fn only_an_operator_marks_another_account_or_keeps_it() {
    // given: two accounts, neither an operator, one asked to be deleted
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let grace = a_verified_account(&wired, GRACE).await;
    wired
        .accounts
        .ask_deletion(&grace, HER_PASSWORD)
        .await
        .expect("her password asks for it");

    // expect: the other may neither mark the first nor keep the second
    assert!(matches!(
        wired.accounts.mark_for_deletion(&ada, &grace, None).await,
        Err(AccountsError::NotAnOperator)
    ));
    assert!(matches!(
        wired.accounts.keep_account_of(&ada, &grace, None).await,
        Err(AccountsError::NotAnOperator)
    ));
    // and: nothing changed
    assert_eq!(pending(&wired, &ada).await, None);
    assert!(pending(&wired, &grace).await.is_some());
}

#[tokio::test]
async fn an_account_asked_to_be_deleted_is_refused_at_signing_in() {
    // given: an account its holder asked to delete
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");

    // when: she signs in
    let refused = wired.accounts.begin_session(ADA, HER_PASSWORD).await;

    // then: it is refused, as a locked account is, until signing in learns to offer keeping it
    assert!(matches!(refused, Err(AccountsError::Locked)));
}
