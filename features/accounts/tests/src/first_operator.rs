use accounts_core::{AccountsCore, AccountsError, EmailAddress, EmailAddresses, SigningUp};

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const GRACE: &str = "grace@example.com";

async fn drawn(wired: &Wired) -> String {
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
    let wired = wiring::wired();
    let code = drawn(&wired).await;
    let accounts = by_invitation(&wired);
    assert!(
        accounts
            .operator_wanted()
            .await
            .expect("asking should succeed")
    );

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

    assert!(
        !accounts
            .operator_wanted()
            .await
            .expect("asking should succeed")
    );
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
async fn a_code_that_is_not_the_logged_one_lets_nobody_in() {
    let wired = wiring::wired();
    drawn(&wired).await;

    assert!(matches!(
        by_invitation(&wired)
            .sign_up_to_operate(ADA, "Ada", HER_PASSWORD, "0123456789abcdef0123456789abcdef")
            .await,
        Err(AccountsError::OperatorCodeWrong)
    ));
    assert!(!held(&wired, ADA).await);
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}

#[tokio::test]
async fn where_anyone_may_sign_up_a_code_changes_nothing() {
    let wired = wiring::wired();

    wired
        .accounts
        .sign_up_to_operate(ADA, "Ada", HER_PASSWORD, "whatever was typed")
        .await
        .expect("an open instance asks no code");

    assert!(held(&wired, ADA).await);
}
