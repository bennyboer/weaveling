use accounts_core::{AccountId, AccountServiceError, EmailAddress, EmailAddresses, Operators};

use crate::wiring::{HER_PASSWORD, Wired, wired};

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

async fn is_operator(wired: &Wired, id: &AccountId) -> bool {
    wired
        .accounts
        .get(id)
        .await
        .expect("the account should exist")
        .state
        .is_operator()
}

#[tokio::test]
async fn the_code_drawn_while_there_is_no_operator_makes_a_verified_account_one() {
    let wired = wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    let code = drawn(&wired).await;

    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code drawn makes an operator");

    assert!(is_operator(&wired, &ada).await);
    assert!(
        wired
            .secrets
            .any_appointed()
            .await
            .expect("reading should succeed"),
        "or the next start would draw a code again"
    );
}

#[tokio::test]
async fn once_there_is_an_operator_no_code_is_drawn() {
    let wired = wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    let code = drawn(&wired).await;
    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code drawn makes an operator");

    assert!(
        wired
            .accounts
            .draw_operator_code()
            .await
            .expect("drawing should succeed")
            .is_none()
    );
}

#[tokio::test]
async fn a_code_makes_one_operator_only() {
    let wired = wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    let grace = signed_up(&wired, "grace@example.com", true).await;
    let code = drawn(&wired).await;
    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code drawn makes an operator");

    assert!(matches!(
        wired.accounts.become_operator(&grace, &code).await,
        Err(AccountServiceError::OperatorCodeWrong)
    ));
    assert!(!is_operator(&wired, &grace).await);
}

#[tokio::test]
async fn a_code_drawn_at_an_earlier_start_is_good_no_longer() {
    let wired = wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    let earlier = drawn(&wired).await;
    let _later = drawn(&wired).await;

    assert!(matches!(
        wired.accounts.become_operator(&ada, &earlier).await,
        Err(AccountServiceError::OperatorCodeWrong)
    ));
}

#[tokio::test]
async fn a_wrong_code_makes_no_operator() {
    let wired = wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    let _code = drawn(&wired).await;

    assert!(matches!(
        wired
            .accounts
            .become_operator(&ada, "not the code at all")
            .await,
        Err(AccountServiceError::OperatorCodeWrong)
    ));
    assert!(!is_operator(&wired, &ada).await);
}

#[tokio::test]
async fn an_unverified_address_is_refused_and_leaves_the_code_for_after_it_is_verified() {
    let wired = wired();
    let ada = signed_up(&wired, "ada@example.com", false).await;
    let code = drawn(&wired).await;

    assert!(matches!(
        wired.accounts.become_operator(&ada, &code).await,
        Err(AccountServiceError::NotVerifiedYet)
    ));

    wired
        .accounts
        .verify(
            "ada@example.com",
            &wired.last_code_sent_to("ada@example.com"),
        )
        .await
        .expect("the code sent should verify");
    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code is still good, being refused for the address and not for itself");
    assert!(is_operator(&wired, &ada).await);
}
