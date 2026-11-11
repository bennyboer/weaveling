use accounts_core::{AccountsCore, AccountsError, EmailAddress, EmailAddresses, SigningUp};

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const GRACE: &str = "grace@example.com";

async fn an_operator_code(wired: &Wired) -> String {
    wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn")
        .as_str()
        .to_owned()
}

fn by_invitation(wired: &Wired) -> AccountsCore {
    wired.accounts.clone().signing_up(SigningUp::ByInvitation)
}

async fn held(wired: &Wired, address: &str) -> bool {
    wired
        .secrets
        .holder(&EmailAddress::new(address).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .is_some()
}

#[tokio::test]
async fn where_signing_up_is_by_invitation_the_code_from_the_log_lets_the_first_account_in_to_operate()
 {
    // given: an instance signing up by invitation with an operator code drawn
    let wired = wiring::wired();
    let code = an_operator_code(&wired).await;
    let accounts = by_invitation(&wired);

    // then: an operator is wanted
    assert!(
        accounts
            .operator_wanted()
            .await
            .expect("asking should succeed")
    );

    // when: the first account signs up with the code and becomes operator by it
    accounts
        .sign_up_to_operate(ADA, "Ada", HER_PASSWORD, &format!(" {code}\n"))
        .await
        .expect("whoever reads the log may sign up to run the instance");
    accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the address is confirmed as any other");
    let id = accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("signing in should succeed");
    accounts
        .become_operator(&id, &code)
        .await
        .expect("the code was only looked at when signing up, so it is still there to take");

    // then: no operator is wanted any more
    assert!(
        !accounts
            .operator_wanted()
            .await
            .expect("asking should succeed")
    );
    // and: nobody else signs up with the code
    assert!(matches!(
        accounts
            .sign_up_to_operate(GRACE, "Grace", HER_PASSWORD, &code)
            .await,
        Err(AccountsError::SigningUpByInvitationOnly)
    ));
    assert!(
        !held(&wired, GRACE).await,
        "once someone runs the instance, the way in is by invitation alone"
    );
}

#[tokio::test]
async fn the_code_from_the_log_lets_only_the_first_account_in_and_only_it_operate() {
    // given: an instance signing up by invitation with an operator code drawn
    let wired = wiring::wired();
    let code = an_operator_code(&wired).await;
    let accounts = by_invitation(&wired);
    // and: the first account signed up with it
    accounts
        .sign_up_to_operate(ADA, "Ada", HER_PASSWORD, &code)
        .await
        .expect("whoever reads the log may sign up to run the instance");

    // when: another signs up with the same code, before anyone operates
    let second = accounts
        .sign_up_to_operate(GRACE, "Grace", HER_PASSWORD, &code)
        .await;

    // then: it is refused, and no account is made
    assert!(
        matches!(second, Err(AccountsError::OperatorCodeWrong)),
        "whoever reads the log signs up one account by it, not any number: {second:?}"
    );
    assert!(!held(&wired, GRACE).await);
    // and: the first still becomes the operator by it
    accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the address is confirmed as any other");
    let id = accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("signing in should succeed");
    accounts
        .become_operator(&id, &code)
        .await
        .expect("the code is held for her");
}

#[tokio::test]
async fn a_code_held_for_one_makes_nobody_else_the_operator() {
    // given: an instance where anyone may sign up, its code held for the first to sign up with it
    let wired = wiring::wired();
    let code = an_operator_code(&wired).await;
    let by_invitation = by_invitation(&wired);
    by_invitation
        .sign_up_to_operate(ADA, "Ada", HER_PASSWORD, &code)
        .await
        .expect("the first signs up with the code");
    // and: another account, signed up and confirmed
    let accounts = &wired.accounts;
    accounts
        .sign_up(GRACE, "Grace", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    accounts
        .verify(GRACE, &wired.last_code_sent_to(GRACE))
        .await
        .expect("the address is confirmed");
    let grace = accounts
        .sign_in(GRACE, HER_PASSWORD)
        .await
        .expect("signing in should succeed");

    // when: the other tries to become the operator with the code
    let refused = accounts.become_operator(&grace, &code).await;

    // then: it is refused
    assert!(
        matches!(refused, Err(AccountsError::OperatorCodeWrong)),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_code_that_is_not_the_logged_one_lets_nobody_in() {
    // given: an instance signing up by invitation with an operator code drawn
    let wired = wiring::wired();
    an_operator_code(&wired).await;

    // then: signing up to operate with another code is refused
    assert!(matches!(
        by_invitation(&wired)
            .sign_up_to_operate(ADA, "Ada", HER_PASSWORD, "0123456789abcdef0123456789abcdef")
            .await,
        Err(AccountsError::OperatorCodeWrong)
    ));
    // and: no account is made and nothing is sent
    assert!(!held(&wired, ADA).await);
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}

#[tokio::test]
async fn where_anyone_may_sign_up_a_code_changes_nothing() {
    // given: an instance anyone may sign up to
    let wired = wiring::wired();

    // when: an account signs up to operate with any code
    wired
        .accounts
        .sign_up_to_operate(ADA, "Ada", HER_PASSWORD, "whatever was typed")
        .await
        .expect("an open instance asks no code");

    // then: it is made
    assert!(held(&wired, ADA).await);
}
