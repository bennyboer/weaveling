use accounts_core::{
    AccountCatalog, AccountId, AccountSummary, EmailAddress, EmailAddresses, LockKind,
};

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
        .secrets
        .holder(&EmailAddress::new(email).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

async fn verified(wired: &Wired, email: &str) {
    wired
        .accounts
        .verify(email, &wired.last_code_sent_to(email))
        .await
        .expect("the code sent should verify");
}

async fn listed(wired: &Wired) -> Vec<AccountSummary> {
    wired.settle().await;

    wired
        .catalog
        .listed_after(None, 100)
        .await
        .expect("listing should succeed")
}

async fn summary_of(wired: &Wired, id: &AccountId) -> AccountSummary {
    listed(wired)
        .await
        .into_iter()
        .find(|summary| summary.id == *id)
        .expect("the account should be listed")
}

#[tokio::test]
async fn an_account_is_listed_once_it_has_signed_up() {
    let wired = wiring::wired();

    let ada = signed_up(&wired, ADA).await;

    let summary = summary_of(&wired, &ada).await;
    assert_eq!(summary.email.to_string(), ADA);
    assert_eq!(summary.name.to_string(), "Someone");
    assert!(!summary.verified);
}

#[tokio::test]
async fn what_happens_to_an_account_reaches_the_list() {
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;
    let grace = signed_up(&wired, GRACE).await;
    verified(&wired, ADA).await;
    verified(&wired, GRACE).await;
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

    wired
        .accounts
        .lock(&grace, &ada, "Sent spam.")
        .await
        .expect("an operator may lock an account");

    let ada_listed = summary_of(&wired, &ada).await;
    assert!(ada_listed.verified);
    assert_eq!(
        ada_listed.locks[&LockKind::Operator].reason.as_str(),
        "Sent spam."
    );
    assert!(summary_of(&wired, &grace).await.operator);

    wired
        .accounts
        .unlock(&grace, &ada)
        .await
        .expect("an operator may unlock an account");
    assert!(summary_of(&wired, &ada).await.locks.is_empty());
}
