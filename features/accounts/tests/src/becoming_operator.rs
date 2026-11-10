use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use accounts_core::{
    AccountId, AccountsCore, AccountsError, Dismissal, EmailAddress, EmailAddresses, Operators,
    SecretStores, SecretsError, TokenDigest,
};
use accounts_secrets::InMemorySecrets;
use async_trait::async_trait;
use transacting::{InMemoryTransactions, Transaction};

use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

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

async fn is_operator(wired: &Wired, id: &AccountId) -> bool {
    wired
        .account(id)
        .await
        .expect("the account should exist")
        .state
        .is_operator()
}

#[tokio::test]
async fn the_code_drawn_while_there_is_no_operator_makes_a_verified_account_one() {
    // given: a verified account
    let wired = wiring::wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    // and: an operator code drawn
    let code = an_operator_code(&wired).await;

    // when: the account becomes operator by the code
    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code drawn makes an operator");

    // then: it is an operator
    assert!(is_operator(&wired, &ada).await);
    // and: the instance knows someone is appointed
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
    // given: an operator
    let wired = wiring::wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    let code = an_operator_code(&wired).await;
    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code drawn makes an operator");

    // then: no code is drawn
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
    // given: two verified accounts
    let wired = wiring::wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    let grace = signed_up(&wired, "grace@example.com", true).await;
    // and: one made operator by a code
    let code = an_operator_code(&wired).await;
    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code drawn makes an operator");

    // then: the code is wrong for the other
    assert!(matches!(
        wired.accounts.become_operator(&grace, &code).await,
        Err(AccountsError::OperatorCodeWrong)
    ));
    // and: the other is no operator
    assert!(!is_operator(&wired, &grace).await);
}

#[tokio::test]
async fn a_code_drawn_at_an_earlier_start_is_good_no_longer() {
    // given: a verified account
    let wired = wiring::wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    // and: an operator code drawn
    let earlier = an_operator_code(&wired).await;
    // and: another drawn after it
    let _later = an_operator_code(&wired).await;

    // then: the earlier code is wrong
    assert!(matches!(
        wired.accounts.become_operator(&ada, &earlier).await,
        Err(AccountsError::OperatorCodeWrong)
    ));
}

#[tokio::test]
async fn a_wrong_code_makes_no_operator() {
    // given: a verified account
    let wired = wiring::wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    // and: an operator code drawn
    let _code = an_operator_code(&wired).await;

    // then: another code is wrong
    assert!(matches!(
        wired
            .accounts
            .become_operator(&ada, "not the code at all")
            .await,
        Err(AccountsError::OperatorCodeWrong)
    ));
    // and: the account is no operator
    assert!(!is_operator(&wired, &ada).await);
}

#[tokio::test]
async fn an_unverified_address_is_refused_and_leaves_the_code_for_after_it_is_verified() {
    // given: an unverified account
    let wired = wiring::wired();
    let ada = signed_up(&wired, "ada@example.com", false).await;
    // and: an operator code drawn
    let code = an_operator_code(&wired).await;

    // then: becoming operator by the code is refused
    assert!(matches!(
        wired.accounts.become_operator(&ada, &code).await,
        Err(AccountsError::NotVerifiedYet)
    ));

    // when: the account is verified
    wired
        .accounts
        .verify(
            "ada@example.com",
            &wired.last_code_sent_to("ada@example.com"),
        )
        .await
        .expect("the code sent should verify");
    // and: it becomes operator by the same code
    wired
        .accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code is still good, being refused for the address and not for itself");

    // then: it is an operator
    assert!(is_operator(&wired, &ada).await);
}

struct FailingToAppointOnce {
    kept: Arc<InMemorySecrets>,
    armed: AtomicBool,
}

#[async_trait]
impl Operators for FailingToAppointOnce {
    async fn keep_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError> {
        self.kept.keep_code(transaction, digest).await
    }

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        self.kept.holds_code(digest).await
    }

    async fn take_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError> {
        self.kept.take_code(transaction, digest).await
    }

    async fn appoint(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        if self.armed.swap(false, Ordering::SeqCst) {
            return Err(SecretsError::Backend("the server went down".into()));
        }

        self.kept.appoint(transaction, account).await
    }

    async fn dismiss(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Dismissal, SecretsError> {
        self.kept.dismiss(transaction, account).await
    }

    async fn is_appointed(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<bool, SecretsError> {
        self.kept.is_appointed(transaction, account).await
    }

    async fn any_appointed(&self) -> Result<bool, SecretsError> {
        self.kept.any_appointed().await
    }
}

#[tokio::test]
async fn an_appointment_that_fails_records_nothing_and_leaves_the_code_to_be_used() {
    // given: a verified account
    let wired = wiring::wired();
    let ada = signed_up(&wired, "ada@example.com", true).await;
    // and: an instance whose first appointment fails
    let accounts = AccountsCore::new(
        wired.store.clone(),
        Arc::new(InMemoryTransactions::new()),
        SecretStores {
            operators: Arc::new(FailingToAppointOnce {
                kept: wired.secrets.clone(),
                armed: AtomicBool::new(true),
            }),
            ..SecretStores::kept_by(wired.secrets.clone(), wired.throttles.clone())
        },
        wired.catalog.clone(),
        wired.trash.clone(),
        wired.mailbox.clone(),
        wired.clock.clone(),
    );
    // and: an operator code drawn
    let code = an_operator_code(&wired).await;

    // then: becoming operator fails
    assert!(accounts.become_operator(&ada, &code).await.is_err());
    // and: the account is no operator
    assert!(
        !is_operator(&wired, &ada).await,
        "the event, the code's take and the appointment commit together or not at all"
    );

    // when: it becomes operator by the same code again
    accounts
        .become_operator(&ada, &code)
        .await
        .expect("the code was never used up, so it still makes her an operator");

    // then: it is an operator
    assert!(is_operator(&wired, &ada).await);
    // and: no code is drawn any more
    assert!(
        accounts
            .draw_operator_code()
            .await
            .expect("drawing should succeed")
            .is_none(),
        "she is appointed as well as recorded"
    );
}
