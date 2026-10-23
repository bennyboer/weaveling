use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use accounts_core::{
    AccountId, AccountService, AccountServiceError, EmailAddress, EmailAddresses, PasswordHash,
    Passwords, PendingVerification, SecretStores, SecretsError, SessionToken, Sessions,
};
use accounts_secrets::InMemorySecrets;
use async_trait::async_trait;

use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const HER_NEW_PASSWORD: &str = "a better horse, a better battery";
const RESETTING: &str = "Setting a new Weaveling password";

async fn ada_signed_up(wired: &Wired, confirmed: bool) -> AccountId {
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    if confirmed {
        wired
            .accounts
            .verify(ADA, &wired.last_code_sent_to(ADA))
            .await
            .expect("the code sent should verify");
    }

    wired
        .secrets
        .holder(&EmailAddress::new(ADA).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

fn reset_codes_sent_to(wired: &Wired, address: &str) -> Vec<String> {
    wired
        .mailbox
        .sent_to(address)
        .iter()
        .filter(|mail| mail.subject == RESETTING)
        .filter_map(|mail| {
            mail.body
                .split_whitespace()
                .find(|word| word.len() == 6 && word.bytes().all(|digit| digit.is_ascii_digit()))
                .map(str::to_owned)
        })
        .collect()
}

async fn a_reset_asked_for(wired: &Wired) -> String {
    wired
        .accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("asking should succeed");

    reset_codes_sent_to(wired, ADA)
        .pop()
        .expect("a reset code should have been sent")
}

fn wrong(code: &str) -> &'static str {
    if code == "000000" { "000001" } else { "000000" }
}

#[tokio::test]
async fn asking_to_reset_sends_a_code_for_it_to_the_address() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;

    a_reset_asked_for(&wired).await;

    assert_eq!(reset_codes_sent_to(&wired, ADA).len(), 1);
}

#[tokio::test]
async fn asking_for_an_address_nobody_has_sends_nothing_and_says_nothing() {
    let wired = wiring::wired();

    wired
        .accounts
        .ask_to_reset_password("nobody@example.com")
        .await
        .expect("an unknown address must look like any other, or it gives away who has one");

    assert!(wired.mailbox.sent_to("nobody@example.com").is_empty());
}

#[tokio::test]
async fn an_address_held_but_not_the_accounts_own_gets_no_code() {
    let wired = wiring::wired();
    let id = ada_signed_up(&wired, true).await;
    wired
        .secrets
        .claim(
            &EmailAddress::new("ada@example.org").expect("a plain address"),
            &id,
            wiring::at(1_700_000_000),
        )
        .await
        .expect("claiming should succeed");

    wired
        .accounts
        .ask_to_reset_password("ada@example.org")
        .await
        .expect("asking should not fail");

    assert!(reset_codes_sent_to(&wired, "ada@example.org").is_empty());
}

#[tokio::test]
async fn the_code_sent_sets_the_new_password_and_the_old_one_no_longer_signs_in() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let code = a_reset_asked_for(&wired).await;

    wired
        .accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code sent should reset");

    assert!(wired.accounts.sign_in(ADA, HER_NEW_PASSWORD).await.is_ok());
    assert!(matches!(
        wired.accounts.sign_in(ADA, HER_PASSWORD).await,
        Err(AccountServiceError::SignInRefused)
    ));
}

#[tokio::test]
async fn resetting_confirms_an_address_not_yet_confirmed() {
    let wired = wiring::wired();
    let id = ada_signed_up(&wired, false).await;
    let code = a_reset_asked_for(&wired).await;

    wired
        .accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code sent should reset");

    assert!(
        wired
            .accounts
            .get(&id)
            .await
            .expect("the account should exist")
            .state
            .is_verified()
    );
    assert!(wired.accounts.sign_in(ADA, HER_NEW_PASSWORD).await.is_ok());
}

#[tokio::test]
async fn a_wrong_code_resets_nothing() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let code = a_reset_asked_for(&wired).await;

    assert!(matches!(
        wired
            .accounts
            .reset_password(ADA, wrong(&code), HER_NEW_PASSWORD)
            .await,
        Err(AccountServiceError::CodeWrong)
    ));
    assert!(wired.accounts.sign_in(ADA, HER_PASSWORD).await.is_ok());
}

#[tokio::test]
async fn a_code_to_confirm_the_address_resets_no_password() {
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;

    assert!(matches!(
        wired
            .accounts
            .reset_password(ADA, &wired.last_code_sent_to(ADA), HER_NEW_PASSWORD)
            .await,
        Err(AccountServiceError::CodeWrong)
    ));
}

#[tokio::test]
async fn a_used_reset_code_is_used_up() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let code = a_reset_asked_for(&wired).await;
    wired
        .accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code sent should reset");

    assert!(matches!(
        wired
            .accounts
            .reset_password(ADA, &code, HER_PASSWORD)
            .await,
        Err(AccountServiceError::CodeWrong)
    ));
}

#[tokio::test]
async fn guesses_at_a_reset_code_are_counted_like_any_others() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let code = a_reset_asked_for(&wired).await;
    for _ in 0..PendingVerification::MAX_TRIES {
        let _refused = wired
            .accounts
            .reset_password(ADA, wrong(&code), HER_NEW_PASSWORD)
            .await;
    }

    assert!(matches!(
        wired
            .accounts
            .reset_password(ADA, &code, HER_NEW_PASSWORD)
            .await,
        Err(AccountServiceError::CodeExhausted)
    ));
}

#[tokio::test]
async fn a_new_password_must_be_one_that_would_be_taken_at_signing_up() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let code = a_reset_asked_for(&wired).await;

    assert!(matches!(
        wired.accounts.reset_password(ADA, &code, "short").await,
        Err(AccountServiceError::InvalidPassword(_))
    ));
    wired
        .accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("a password refused for its length is no try at the code");
}

struct FailingOnce {
    kept: Arc<InMemorySecrets>,
    armed: AtomicBool,
}

#[async_trait]
impl Passwords for FailingOnce {
    async fn keep(&self, account: &AccountId, password: &PasswordHash) -> Result<(), SecretsError> {
        if self.armed.swap(false, Ordering::SeqCst) {
            return Err(SecretsError::Backend("the disk was full".into()));
        }

        self.kept.keep(account, password).await
    }

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError> {
        self.kept.of(account).await
    }
}

#[tokio::test]
async fn a_reset_broken_off_after_it_was_told_is_finished_by_the_same_code() {
    let wired = wiring::wired();
    let passwords = Arc::new(FailingOnce {
        kept: wired.secrets.clone(),
        armed: AtomicBool::new(false),
    });
    let accounts = AccountService::new(
        wired.store.clone(),
        SecretStores {
            passwords: passwords.clone(),
            ..SecretStores::kept_by(wired.secrets.clone(), wired.throttles.clone())
        },
        wired.catalog.clone(),
        wired.mailbox.clone(),
        wired.clock.clone(),
    );
    accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("asking should succeed");
    let code = reset_codes_sent_to(&wired, ADA)
        .pop()
        .expect("a reset code should have been sent");

    passwords.armed.store(true, Ordering::SeqCst);
    assert!(
        accounts
            .reset_password(ADA, &code, HER_NEW_PASSWORD)
            .await
            .is_err(),
        "the new password could not be kept"
    );
    assert!(
        accounts.sign_in(ADA, HER_PASSWORD).await.is_ok(),
        "a reset broken off before the new password was kept leaves the old one"
    );

    accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code is left open until the reset is done, so trying again finishes it");

    assert!(accounts.sign_in(ADA, HER_NEW_PASSWORD).await.is_ok());
    assert!(accounts.sign_in(ADA, HER_PASSWORD).await.is_err());
}

async fn a_session(accounts: &AccountService) -> SessionToken {
    accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("signing in should succeed")
        .0
}

async fn still_signed_in(accounts: &AccountService, token: &SessionToken) -> bool {
    accounts
        .signed_in(token)
        .await
        .expect("asking should succeed")
        .is_some()
}

#[tokio::test]
async fn a_reset_ends_every_session_begun_before_it_on_every_device() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let phone = a_session(&wired.accounts).await;
    let laptop = a_session(&wired.accounts).await;

    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::minutes(1));
    let code = a_reset_asked_for(&wired).await;
    wired
        .accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code sent should reset");

    assert!(
        !still_signed_in(&wired.accounts, &phone).await,
        "whoever reset a password because another knew it must have them put out"
    );
    assert!(!still_signed_in(&wired.accounts, &laptop).await);
    assert_eq!(
        wired
            .secrets
            .find(&laptop.digest())
            .await
            .expect("finding should succeed"),
        None,
        "a session found to have ended is ended, not kept"
    );
}

#[tokio::test]
async fn signing_in_the_moment_after_a_reset_counts() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let code = a_reset_asked_for(&wired).await;
    wired
        .accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code sent should reset");

    let fresh = wired
        .accounts
        .begin_session(ADA, HER_NEW_PASSWORD)
        .await
        .expect("the new password should sign in")
        .0;

    assert!(still_signed_in(&wired.accounts, &fresh).await);
}

#[tokio::test]
async fn a_reset_broken_off_after_it_was_told_has_ended_the_sessions_already() {
    let wired = wiring::wired();
    let passwords = Arc::new(FailingOnce {
        kept: wired.secrets.clone(),
        armed: AtomicBool::new(false),
    });
    let accounts = AccountService::new(
        wired.store.clone(),
        SecretStores {
            passwords: passwords.clone(),
            ..SecretStores::kept_by(wired.secrets.clone(), wired.throttles.clone())
        },
        wired.catalog.clone(),
        wired.mailbox.clone(),
        wired.clock.clone(),
    );
    accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");
    let theirs = a_session(&accounts).await;

    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::minutes(1));
    accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("asking should succeed");
    let code = reset_codes_sent_to(&wired, ADA)
        .pop()
        .expect("a reset code should have been sent");
    passwords.armed.store(true, Ordering::SeqCst);
    let _broken_off = accounts.reset_password(ADA, &code, HER_NEW_PASSWORD).await;

    assert!(
        !still_signed_in(&accounts, &theirs).await,
        "the reset is told before its new password is kept, so whoever knew the old one is out \
         even when keeping the new one fails"
    );
}
