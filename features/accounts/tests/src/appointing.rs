use accounts_core::{
    AccountError, AccountId, AccountsError, Dismissal, EmailAddress, EmailAddresses, Operators,
};
use eventsourcing::ServiceError;

use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";

async fn signed_up(wired: &Wired, email: &str, verified: bool) -> AccountId {
    wired
        .accounts
        .sign_up(email, "Someone", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    if verified {
        wired
            .accounts
            .verify(email, &wired.last_code_sent_to(email))
            .await
            .expect("the code sent should verify");
    }

    wired
        .secrets
        .holder(&EmailAddress::new(email).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

async fn grace_the_operator_and_ada(wired: &Wired, ada_verified: bool) -> (AccountId, AccountId) {
    let ada = signed_up(wired, ADA, ada_verified).await;
    let grace = signed_up(wired, GRACE, true).await;
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

async fn is_operator(wired: &Wired, id: &AccountId) -> bool {
    wired
        .account(id)
        .await
        .expect("the account should exist")
        .state
        .is_operator()
}

#[tokio::test]
async fn an_operator_makes_another_account_an_operator_who_may_then_act_as_one() {
    let wired = wiring::wired();
    let (ada, grace) = grace_the_operator_and_ada(&wired, true).await;

    wired
        .accounts
        .appoint_operator(&grace, &ada)
        .await
        .expect("an operator may make another");

    assert!(is_operator(&wired, &ada).await);
    wired
        .accounts
        .list(&ada, None, 10)
        .await
        .expect("a new operator may list the accounts");
    assert_eq!(
        wired
            .secrets
            .dismiss(&grace)
            .await
            .expect("dismissing should succeed"),
        Dismissal::Dismissed,
        "ada is appointed as well, so grace is no longer the last"
    );
}

#[tokio::test]
async fn an_operator_takes_the_role_away_and_it_counts_at_once() {
    let wired = wiring::wired();
    let (ada, grace) = grace_the_operator_and_ada(&wired, true).await;
    wired
        .accounts
        .appoint_operator(&grace, &ada)
        .await
        .expect("an operator may make another");

    wired
        .accounts
        .dismiss_operator(&grace, &ada)
        .await
        .expect("an operator may take the role away again");

    assert!(!is_operator(&wired, &ada).await);
    assert!(matches!(
        wired.accounts.list(&ada, None, 10).await,
        Err(AccountsError::NotAnOperator)
    ));
}

#[tokio::test]
async fn an_operator_may_give_up_the_role_but_not_as_the_last() {
    let wired = wiring::wired();
    let (ada, grace) = grace_the_operator_and_ada(&wired, true).await;

    assert!(matches!(
        wired.accounts.dismiss_operator(&grace, &grace).await,
        Err(AccountsError::LastOperator)
    ));
    assert!(
        is_operator(&wired, &grace).await,
        "the role is taken from the table before it is told, so a refusal tells nothing"
    );

    wired
        .accounts
        .appoint_operator(&grace, &ada)
        .await
        .expect("an operator may make another");
    wired
        .accounts
        .dismiss_operator(&grace, &grace)
        .await
        .expect("with another operator she may give it up");
    assert!(!is_operator(&wired, &grace).await);
}

#[tokio::test]
async fn only_an_operator_makes_or_unmakes_one() {
    let wired = wiring::wired();
    let (ada, grace) = grace_the_operator_and_ada(&wired, true).await;

    assert!(matches!(
        wired.accounts.appoint_operator(&ada, &ada).await,
        Err(AccountsError::NotAnOperator)
    ));
    assert!(matches!(
        wired.accounts.dismiss_operator(&ada, &grace).await,
        Err(AccountsError::NotAnOperator)
    ));
    assert!(is_operator(&wired, &grace).await);
    assert!(!is_operator(&wired, &ada).await);
}

#[tokio::test]
async fn an_account_refusing_the_role_is_not_appointed() {
    let wired = wiring::wired();
    let (ada, grace) = grace_the_operator_and_ada(&wired, false).await;

    assert!(matches!(
        wired.accounts.appoint_operator(&grace, &ada).await,
        Err(AccountsError::Events(ServiceError::Refused(
            AccountError::NotVerifiedYet
        )))
    ));
    assert_eq!(
        wired
            .secrets
            .dismiss(&grace)
            .await
            .expect("dismissing should succeed"),
        Dismissal::LastOne,
        "the account refuses before anyone is appointed, so grace is still the only one"
    );
}

#[tokio::test]
async fn a_locked_account_is_not_made_an_operator() {
    let wired = wiring::wired();
    let (ada, grace) = grace_the_operator_and_ada(&wired, true).await;
    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    assert!(matches!(
        wired.accounts.appoint_operator(&grace, &ada).await,
        Err(AccountsError::Events(ServiceError::Refused(
            AccountError::LockedCannotBecomeOperator
        )))
    ));
    assert!(!is_operator(&wired, &ada).await);
}
