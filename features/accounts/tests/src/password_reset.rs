use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use accounts_core::{
    AccountId, AccountsCore, AccountsError, EmailAddress, EmailAddresses, PasswordHash, Passwords,
    PendingVerification, SecretStores, SecretsError, SessionToken, Sessions,
};
use accounts_secrets::InMemorySecrets;
use async_trait::async_trait;

use time::Duration;
use tokio::sync::{Barrier, Notify};
use transacting::{Failure, InMemoryTransactions, Transaction, Transactions};

use crate::session_ends;
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
        Err(AccountsError::SignInRefused)
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
            .account(&id)
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
        Err(AccountsError::CodeWrong)
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
        Err(AccountsError::CodeWrong)
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
        Err(AccountsError::CodeWrong)
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
        Err(AccountsError::CodeExhausted)
    ));
}

#[tokio::test]
async fn a_new_password_must_be_one_that_would_be_taken_at_signing_up() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let code = a_reset_asked_for(&wired).await;

    assert!(matches!(
        wired.accounts.reset_password(ADA, &code, "short").await,
        Err(AccountsError::InvalidPassword(_))
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
    async fn keep(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError> {
        if self.armed.swap(false, Ordering::SeqCst) {
            return Err(SecretsError::Backend("the disk was full".into()));
        }

        self.kept.keep(transaction, account, password).await
    }

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError> {
        self.kept.of(account).await
    }
}

#[tokio::test]
async fn a_reset_whose_new_password_cannot_be_kept_changes_nothing_and_the_same_code_finishes_it() {
    let wired = wiring::wired();
    let passwords = Arc::new(FailingOnce {
        kept: wired.secrets.clone(),
        armed: AtomicBool::new(false),
    });
    let accounts = AccountsCore::new(
        wired.store.clone(),
        Arc::new(InMemoryTransactions::new()),
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
        "a reset whose new password was not kept is not told either, so the old one stands"
    );
    assert!(accounts.sign_in(ADA, HER_NEW_PASSWORD).await.is_err());

    accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code is left open until the reset is done, so trying again finishes it");

    assert!(accounts.sign_in(ADA, HER_NEW_PASSWORD).await.is_ok());
    assert!(accounts.sign_in(ADA, HER_PASSWORD).await.is_err());
}

async fn a_session(accounts: &AccountsCore) -> SessionToken {
    accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("signing in should succeed")
        .0
}

async fn still_signed_in(accounts: &AccountsCore, token: &SessionToken) -> bool {
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
async fn a_reset_whose_new_password_cannot_be_kept_ends_no_session_until_it_is_finished() {
    let wired = wiring::wired();
    let passwords = Arc::new(FailingOnce {
        kept: wired.secrets.clone(),
        armed: AtomicBool::new(false),
    });
    let accounts = AccountsCore::new(
        wired.store.clone(),
        Arc::new(InMemoryTransactions::new()),
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
    let broken_off = accounts.reset_password(ADA, &code, HER_NEW_PASSWORD).await;
    let after_breaking_off = still_signed_in(&accounts, &theirs).await;
    accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the same code finishes the reset");

    assert!(broken_off.is_err());
    assert!(
        after_breaking_off,
        "the reset and its new password are one, so a reset not done ends nothing, as the old \
         password still lets its holder in"
    );
    assert!(
        !still_signed_in(&accounts, &theirs).await,
        "the reset done ends whoever knew the old password"
    );
}

#[tokio::test]
async fn a_reset_reaches_the_end_of_the_sessions_begun_before_it_at_once() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    let (token, signed_in) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in before resetting");
    let waiting =
        session_ends::awaited(wired.accounts.session_end(&token, signed_in.session.until)).await;
    let code = a_reset_asked_for(&wired).await;
    wired
        .clock
        .set(signed_in.session.begun + Duration::minutes(5));

    wired
        .accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the code sent should reset");

    assert!(session_ends::reached(waiting).await);
}

struct MeetingBeforeEachBegin {
    kept: InMemoryTransactions,
    both: Barrier,
    armed: AtomicBool,
}

#[async_trait]
impl Transactions for MeetingBeforeEachBegin {
    async fn begin(&self) -> Result<Transaction, Failure> {
        if self.armed.load(Ordering::SeqCst) {
            self.both.wait().await;
        }

        self.kept.begin().await
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn one_reset_code_resets_once_however_many_use_it_at_the_same_time() {
    let wired = wiring::wired();
    let transactions = Arc::new(MeetingBeforeEachBegin {
        kept: InMemoryTransactions::new(),
        both: Barrier::new(2),
        armed: AtomicBool::new(false),
    });
    let accounts = AccountsCore::new(
        wired.store.clone(),
        transactions.clone(),
        SecretStores::kept_by(wired.secrets.clone(), wired.throttles.clone()),
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
    accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("asking should succeed");
    let code = reset_codes_sent_to(&wired, ADA)
        .pop()
        .expect("a reset code should have been sent");

    transactions.armed.store(true, Ordering::SeqCst);
    let resetting = |password: &'static str| {
        let accounts = accounts.clone();
        let code = code.clone();
        tokio::spawn(async move { accounts.reset_password(ADA, &code, password).await })
    };
    let (first, second) = (
        resetting(HER_NEW_PASSWORD),
        resetting("yet another long password"),
    );
    let answers = tokio::time::timeout(Duration::seconds(10).unsigned_abs(), async {
        [
            first.await.expect("the first reset should not panic"),
            second.await.expect("the second reset should not panic"),
        ]
    })
    .await
    .expect("both resets come to an end");

    assert_eq!(
        answers.iter().filter(|answer| answer.is_ok()).count(),
        1,
        "two resets that both got past the check still use the code once: {answers:?}"
    );
    assert!(
        answers
            .iter()
            .any(|answer| matches!(answer, Err(AccountsError::CodeWrong))),
        "the one that came second finds the code gone: {answers:?}"
    );
}

struct ReadWhileReset {
    kept: Arc<InMemorySecrets>,
    armed: AtomicBool,
    read: Notify,
    go_on: Notify,
}

#[async_trait]
impl Passwords for ReadWhileReset {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError> {
        self.kept.keep(transaction, account, password).await
    }

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError> {
        let kept = self.kept.of(account).await;
        if self.armed.swap(false, Ordering::SeqCst) {
            self.read.notify_one();
            self.go_on.notified().await;
        }

        kept
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_begun_while_the_password_is_reset_does_not_outlive_the_reset() {
    let wired = wiring::wired();
    let passwords = Arc::new(ReadWhileReset {
        kept: wired.secrets.clone(),
        armed: AtomicBool::new(false),
        read: Notify::new(),
        go_on: Notify::new(),
    });
    let accounts = AccountsCore::new(
        wired.store.clone(),
        Arc::new(InMemoryTransactions::new()),
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
    accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("asking should succeed");
    let code = reset_codes_sent_to(&wired, ADA)
        .pop()
        .expect("a reset code should have been sent");

    passwords.armed.store(true, Ordering::SeqCst);
    let signing_in = tokio::spawn({
        let accounts = accounts.clone();
        async move { accounts.begin_session(ADA, HER_PASSWORD).await }
    });
    passwords.read.notified().await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::minutes(1));
    accounts
        .reset_password(ADA, &code, HER_NEW_PASSWORD)
        .await
        .expect("the reset lands while the old password is being checked");
    passwords.go_on.notify_one();
    let (token, _) = tokio::time::timeout(Duration::seconds(10).unsigned_abs(), signing_in)
        .await
        .expect("signing in comes to an end")
        .expect("signing in should not panic")
        .expect("the old password was still the one read");

    assert!(
        !still_signed_in(&accounts, &token).await,
        "a session begun before the reset landed ends with the others"
    );
}
