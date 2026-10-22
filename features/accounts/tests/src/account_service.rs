use std::sync::Arc;

use accounts_core::{
    AccountEvent, AccountId, AccountService, AccountServiceError, EmailAddress, EmailAddresses,
    KIND, Passwords, PendingVerification,
};
use async_trait::async_trait;
use eventsourcing::{AggregateId, AggregateType, EventStore, Recorded, StoreError, Version};
use time::Duration;

use crate::wiring::{ADA, HER_PASSWORD, Wired, at, wired};

async fn ada_signed_up(wired: &Wired) -> AccountId {
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    wired
        .secrets
        .holder(&EmailAddress::new(ADA).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

async fn is_verified(wired: &Wired, id: &AccountId) -> bool {
    wired
        .accounts
        .get(id)
        .await
        .expect("the account should exist")
        .state
        .is_verified()
}

#[tokio::test]
async fn signing_up_sends_a_code_to_the_address_and_leaves_it_unverified() {
    let wired = wired();

    let id = ada_signed_up(&wired).await;

    assert_eq!(wired.codes_sent_to(ADA).len(), 1);
    assert!(!is_verified(&wired, &id).await);
}

#[tokio::test]
async fn an_account_signs_itself_up() {
    let wired = wired();

    let id = ada_signed_up(&wired).await;

    let recorded = wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(recorded[0].metadata.agent, id.acting());
}

#[tokio::test]
async fn the_password_is_kept_only_as_a_hash_that_admits_it() {
    let wired = wired();

    let id = ada_signed_up(&wired).await;

    let kept = wired
        .secrets
        .of(&id)
        .await
        .expect("reading should succeed")
        .expect("a password should be kept");

    assert!(kept.admits(HER_PASSWORD));
    assert!(!kept.as_str().contains(HER_PASSWORD));
}

#[tokio::test]
async fn the_code_that_was_sent_verifies_the_address() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;

    wired
        .accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");

    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn a_used_code_is_used_up() {
    let wired = wired();
    ada_signed_up(&wired).await;
    let code = wired.last_code_sent_to(ADA);
    wired
        .accounts
        .verify(ADA, &code)
        .await
        .expect("the code sent should verify");

    assert!(matches!(
        wired.accounts.verify(ADA, &code).await,
        Err(AccountServiceError::CodeWrong)
    ));
}

#[tokio::test]
async fn the_address_is_verified_however_it_is_typed() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;

    wired
        .accounts
        .verify("  ADA@Example.com ", &wired.last_code_sent_to(ADA))
        .await
        .expect("the same address in other letters should verify");

    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn a_wrong_code_is_refused_and_leaves_the_address_unverified() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;
    let wrong = if wired.last_code_sent_to(ADA) == "000000" {
        "000001"
    } else {
        "000000"
    };

    assert!(matches!(
        wired.accounts.verify(ADA, wrong).await,
        Err(AccountServiceError::CodeWrong)
    ));
    assert!(!is_verified(&wired, &id).await);
}

async fn guessed_wrong(wired: &Wired, times: u32) -> String {
    let right = wired.last_code_sent_to(ADA);
    let wrong = if right == "000000" {
        "000001"
    } else {
        "000000"
    };

    for _ in 0..times {
        let _refused = wired.accounts.verify(ADA, wrong).await;
    }

    right
}

#[tokio::test]
async fn one_guess_short_of_the_limit_the_right_code_still_counts() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;

    let right = guessed_wrong(&wired, PendingVerification::MAX_TRIES - 1).await;

    wired
        .accounts
        .verify(ADA, &right)
        .await
        .expect("the right code should verify");
    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn after_too_many_wrong_codes_not_even_the_right_one_counts() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;

    let right = guessed_wrong(&wired, PendingVerification::MAX_TRIES).await;

    assert!(matches!(
        wired.accounts.verify(ADA, &right).await,
        Err(AccountServiceError::CodeExhausted)
    ));
    assert!(
        !is_verified(&wired, &id).await,
        "six digits are a million guesses, so the guesses at one code must be few"
    );
}

#[tokio::test]
async fn a_code_lasts_a_quarter_of_an_hour() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;
    let code = wired.last_code_sent_to(ADA);

    wired
        .clock
        .set(at(1_700_000_000) + Duration::minutes(15) + Duration::seconds(1));
    assert!(matches!(
        wired.accounts.verify(ADA, &code).await,
        Err(AccountServiceError::CodeExpired)
    ));
    assert!(!is_verified(&wired, &id).await);

    wired.clock.set(at(1_700_000_000) + Duration::minutes(15));
    wired
        .accounts
        .verify(ADA, &code)
        .await
        .expect("to the second it was given, the code still counts");
}

#[tokio::test]
async fn asking_again_sends_a_new_code_and_the_old_one_no_longer_counts() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;
    let old = wired.last_code_sent_to(ADA);

    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("asking again should succeed");
    let new = wired.last_code_sent_to(ADA);
    assert_eq!(wired.codes_sent_to(ADA).len(), 2);

    if old != new {
        assert!(matches!(
            wired.accounts.verify(ADA, &old).await,
            Err(AccountServiceError::CodeWrong)
        ));
    }
    wired
        .accounts
        .verify(ADA, &new)
        .await
        .expect("the new code should verify");
    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn asking_again_once_verified_or_for_an_address_nobody_has_sends_nothing() {
    let wired = wired();
    ada_signed_up(&wired).await;
    wired
        .accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");

    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("asking again should not fail");
    wired
        .accounts
        .send_code_again("nobody@example.com")
        .await
        .expect("asking for an unknown address should not say so");

    assert_eq!(wired.codes_sent_to(ADA).len(), 1);
    assert!(wired.mailbox.sent_to("nobody@example.com").is_empty());
}

#[tokio::test]
async fn signing_up_with_an_address_already_taken_makes_no_account_and_tells_its_owner() {
    let wired = wired();
    let id = ada_signed_up(&wired).await;

    wired
        .accounts
        .sign_up("Ada@Example.com", "Somebody else", "another long password")
        .await
        .expect("a taken address must look like any other sign-up, or it gives away who has one");

    assert_eq!(
        wired
            .secrets
            .holder(&EmailAddress::new(ADA).expect("a plain address"))
            .await
            .expect("reading should succeed"),
        Some(id)
    );
    assert!(
        wired
            .secrets
            .of(&id)
            .await
            .expect("reading should succeed")
            .is_some_and(|kept| kept.admits(HER_PASSWORD)),
        "the owner's password is not replaced by the stranger's"
    );
    assert_eq!(
        wired.told.waiting_to_be_published(),
        1,
        "a second account, even one racing for the address, would keep the owner's address in events of its own"
    );
    assert_eq!(wired.codes_sent_to(ADA).len(), 1, "no second code");
    assert_eq!(
        wired.mailbox.sent_to(ADA).len(),
        2,
        "the owner hears that someone tried their address"
    );
}

#[tokio::test]
async fn verifying_an_address_nobody_signed_up_with_is_just_a_wrong_code() {
    let wired = wired();

    assert!(matches!(
        wired.accounts.verify("nobody@example.com", "123456").await,
        Err(AccountServiceError::CodeWrong)
    ));
}

#[tokio::test]
async fn what_is_given_is_checked_before_anything_is_kept() {
    let wired = wired();

    for (email, name, password) in [
        ("ada", "Ada", HER_PASSWORD),
        (ADA, " ", HER_PASSWORD),
        (ADA, "Ada", "short"),
    ] {
        assert!(wired.accounts.sign_up(email, name, password).await.is_err());
    }

    assert_eq!(
        wired
            .secrets
            .holder(&EmailAddress::new(ADA).expect("a plain address"))
            .await
            .expect("reading should succeed"),
        None
    );
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}

struct Refusing;

#[async_trait]
impl EventStore<AccountEvent> for Refusing {
    async fn append(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        _expected: Version,
        _events: &[Recorded<AccountEvent>],
    ) -> Result<(), StoreError> {
        Err(StoreError::backend(
            aggregate,
            kind,
            "the database went away",
        ))
    }

    async fn read_from(
        &self,
        _aggregate: &AggregateId,
        _kind: AggregateType,
        _from: Version,
    ) -> Result<Vec<Recorded<AccountEvent>>, StoreError> {
        Ok(Vec::new())
    }

    async fn read_through(
        &self,
        _aggregate: &AggregateId,
        _kind: AggregateType,
        _from: Version,
        _through: Version,
    ) -> Result<Vec<Recorded<AccountEvent>>, StoreError> {
        Ok(Vec::new())
    }

    async fn latest_snapshot(
        &self,
        _aggregate: &AggregateId,
        _kind: AggregateType,
    ) -> Result<Option<Recorded<AccountEvent>>, StoreError> {
        Ok(None)
    }

    async fn snapshot_at_or_before(
        &self,
        _aggregate: &AggregateId,
        _kind: AggregateType,
        _version: Version,
    ) -> Result<Option<Recorded<AccountEvent>>, StoreError> {
        Ok(None)
    }

    async fn prune_through(
        &self,
        _aggregate: &AggregateId,
        _kind: AggregateType,
        _through: Version,
    ) -> Result<(), StoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn a_sign_up_whose_account_cannot_be_kept_frees_its_address_again() {
    let wired = wired();
    let accounts = AccountService::new(
        Arc::new(Refusing),
        wired.secrets.clone(),
        wired.secrets.clone(),
        wired.secrets.clone(),
        wired.secrets.clone(),
        wired.mailbox.clone(),
        wired.clock.clone(),
    );

    assert!(accounts.sign_up(ADA, "Ada", HER_PASSWORD).await.is_err());

    assert_eq!(
        wired
            .secrets
            .holder(&EmailAddress::new(ADA).expect("a plain address"))
            .await
            .expect("reading should succeed"),
        None,
        "an address claimed for an account that never came to be would be blocked for good"
    );
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}
