use std::sync::Arc;

use accounts_core::{
    AccountEvent, AccountId, AccountsCore, AccountsError, EmailAddress, EmailAddresses, KIND,
    Passwords, PendingVerification, SecretStores,
};
use async_trait::async_trait;
use eventsourcing::{AggregateId, AggregateType, EventStore, Recorded, StoreError, Version};
use time::Duration;
use transacting::{InMemoryTransactions, Transaction};

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

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
        .account(id)
        .await
        .expect("the account should exist")
        .state
        .is_verified()
}

#[tokio::test]
async fn signing_up_sends_a_code_to_the_address_and_leaves_it_unverified() {
    // given: a fresh instance
    let wired = wiring::wired();

    // when: an account signs up
    let id = ada_signed_up(&wired).await;

    // then: one code is sent to its address
    assert_eq!(wired.codes_sent_to(ADA).len(), 1);
    // and: the address is unverified
    assert!(!is_verified(&wired, &id).await);
}

#[tokio::test]
async fn an_account_signs_itself_up() {
    // given: a fresh instance
    let wired = wiring::wired();

    // when: an account signs up
    let id = ada_signed_up(&wired).await;

    // then: the account itself is the agent of its sign up
    let recorded = wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(recorded[0].metadata.agent, id.acting());
}

#[tokio::test]
async fn the_password_is_kept_only_as_a_hash_that_admits_it() {
    // given: a fresh instance
    let wired = wiring::wired();

    // when: an account signs up
    let id = ada_signed_up(&wired).await;

    // then: what is kept admits the password but does not hold it
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
    // given: an account signed up
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;

    // when: it verifies with the code sent
    wired
        .accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");

    // then: its address is verified
    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn a_used_code_is_used_up() {
    // given: an account verified with the code sent
    let wired = wiring::wired();
    ada_signed_up(&wired).await;
    let code = wired.last_code_sent_to(ADA);
    wired
        .accounts
        .verify(ADA, &code)
        .await
        .expect("the code sent should verify");

    // then: the code is wrong when used again
    assert!(matches!(
        wired.accounts.verify(ADA, &code).await,
        Err(AccountsError::CodeWrong)
    ));
}

#[tokio::test]
async fn the_address_is_verified_however_it_is_typed() {
    // given: an account signed up
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;

    // when: it verifies with its address typed otherwise
    wired
        .accounts
        .verify("  ADA@Example.com ", &wired.last_code_sent_to(ADA))
        .await
        .expect("the same address in other letters should verify");

    // then: its address is verified
    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn a_wrong_code_is_refused_and_leaves_the_address_unverified() {
    // given: an account signed up
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;
    // and: a code other than the one sent
    let wrong = if wired.last_code_sent_to(ADA) == "000000" {
        "000001"
    } else {
        "000000"
    };

    // then: verifying with it is refused
    assert!(matches!(
        wired.accounts.verify(ADA, wrong).await,
        Err(AccountsError::CodeWrong)
    ));
    // and: the address is unverified
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
    // given: an account signed up
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;
    // and: one wrong guess fewer than allowed
    let right = guessed_wrong(&wired, PendingVerification::MAX_TRIES - 1).await;

    // when: it verifies with the right code
    wired
        .accounts
        .verify(ADA, &right)
        .await
        .expect("the right code should verify");

    // then: its address is verified
    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn after_too_many_wrong_codes_not_even_the_right_one_counts() {
    // given: an account signed up
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;
    // and: as many wrong guesses as allowed
    let right = guessed_wrong(&wired, PendingVerification::MAX_TRIES).await;

    // then: the right code is refused
    assert!(matches!(
        wired.accounts.verify(ADA, &right).await,
        Err(AccountsError::CodeExhausted)
    ));
    // and: the address is unverified
    assert!(
        !is_verified(&wired, &id).await,
        "six digits are a million guesses, so the guesses at one code must be few"
    );
}

#[tokio::test]
async fn a_code_lasts_a_quarter_of_an_hour() {
    // given: an account signed up with a code sent
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;
    let code = wired.last_code_sent_to(ADA);

    // when: a second more than a quarter of an hour has gone by
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::minutes(15) + Duration::seconds(1));

    // then: the code has expired
    assert!(matches!(
        wired.accounts.verify(ADA, &code).await,
        Err(AccountsError::CodeExpired)
    ));
    // and: the address is unverified
    assert!(!is_verified(&wired, &id).await);

    // when: exactly a quarter of an hour has gone by
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::minutes(15));

    // then: the code still verifies
    wired
        .accounts
        .verify(ADA, &code)
        .await
        .expect("to the second it was given, the code still counts");
}

#[tokio::test]
async fn asking_again_sends_a_new_code_and_the_old_one_no_longer_counts() {
    // given: an account signed up with a code sent
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;
    let old = wired.last_code_sent_to(ADA);

    // when: it asks for a code again
    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("asking again should succeed");

    // then: a second code is sent
    let new = wired.last_code_sent_to(ADA);
    assert_eq!(wired.codes_sent_to(ADA).len(), 2);
    // and: the old one is wrong
    if old != new {
        assert!(matches!(
            wired.accounts.verify(ADA, &old).await,
            Err(AccountsError::CodeWrong)
        ));
    }
    // and: the new one verifies
    wired
        .accounts
        .verify(ADA, &new)
        .await
        .expect("the new code should verify");
    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn asking_again_once_verified_or_for_an_address_nobody_has_sends_nothing() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired).await;
    wired
        .accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");

    // when: a code is asked for again for its address
    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("asking again should not fail");
    // and: for an address nobody has
    wired
        .accounts
        .send_code_again("nobody@example.com")
        .await
        .expect("asking for an unknown address should not say so");

    // then: no further code is sent to the account
    assert_eq!(wired.codes_sent_to(ADA).len(), 1);
    // and: nothing to the other address
    assert!(wired.mailbox.sent_to("nobody@example.com").is_empty());
}

#[tokio::test]
async fn signing_up_with_an_address_already_taken_makes_no_account_and_tells_its_owner() {
    // given: an account signed up
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;

    // when: someone else signs up with its address
    wired
        .accounts
        .sign_up("Ada@Example.com", "Somebody else", "another long password")
        .await
        .expect("a taken address must look like any other sign-up, or it gives away who has one");

    // then: the address stays the account's
    assert_eq!(
        wired
            .secrets
            .holder(&EmailAddress::new(ADA).expect("a plain address"))
            .await
            .expect("reading should succeed"),
        Some(id)
    );
    // and: so does its password
    assert!(
        wired
            .secrets
            .of(&id)
            .await
            .expect("reading should succeed")
            .is_some_and(|kept| kept.admits(HER_PASSWORD)),
        "the owner's password is not replaced by the stranger's"
    );
    // and: no second account is told of
    assert_eq!(
        wired.told.waiting_to_be_published(),
        1,
        "a second account, even one racing for the address, would keep the owner's address in events of its own"
    );
    // and: no second code is sent
    assert_eq!(wired.codes_sent_to(ADA).len(), 1, "no second code");
    // and: the owner is told of the attempt
    assert_eq!(
        wired.mailbox.sent_to(ADA).len(),
        2,
        "the owner hears that someone tried their address"
    );
}

#[tokio::test]
async fn verifying_an_address_nobody_signed_up_with_is_just_a_wrong_code() {
    // given: a fresh instance
    let wired = wiring::wired();

    // then: verifying an address nobody has is a wrong code
    assert!(matches!(
        wired.accounts.verify("nobody@example.com", "123456").await,
        Err(AccountsError::CodeWrong)
    ));
}

#[tokio::test]
async fn what_is_given_is_checked_before_anything_is_kept() {
    // given: a fresh instance
    let wired = wiring::wired();

    // then: a sign up with a bad address, name or password is refused
    for (email, name, password) in [
        ("ada", "Ada", HER_PASSWORD),
        (ADA, " ", HER_PASSWORD),
        (ADA, "Ada", "short"),
    ] {
        assert!(wired.accounts.sign_up(email, name, password).await.is_err());
    }

    // and: the address is not held
    assert_eq!(
        wired
            .secrets
            .holder(&EmailAddress::new(ADA).expect("a plain address"))
            .await
            .expect("reading should succeed"),
        None
    );
    // and: nothing is sent
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}

struct Refusing;

#[async_trait]
impl EventStore<AccountEvent> for Refusing {
    async fn append(
        &self,
        _transaction: &mut Transaction,
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

    async fn read_from_within(
        &self,
        _transaction: &mut Transaction,
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

    async fn latest_snapshot_within(
        &self,
        _transaction: &mut Transaction,
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
        _transaction: &mut Transaction,
        _aggregate: &AggregateId,
        _kind: AggregateType,
        _through: Version,
    ) -> Result<(), StoreError> {
        Ok(())
    }

    async fn forget(
        &self,
        _transaction: &mut Transaction,
        _aggregate: &AggregateId,
        _kind: AggregateType,
    ) -> Result<(), StoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn a_sign_up_whose_account_cannot_be_kept_frees_its_address_again() {
    // given: an instance whose accounts cannot be kept
    let wired = wiring::wired();
    let accounts = AccountsCore::new(
        Arc::new(Refusing),
        Arc::new(InMemoryTransactions::new()),
        SecretStores::kept_by(wired.secrets.clone(), wired.throttles.clone()),
        wired.catalog.clone(),
        wired.mailbox.clone(),
        wired.clock.clone(),
    );

    // then: a sign up fails
    assert!(accounts.sign_up(ADA, "Ada", HER_PASSWORD).await.is_err());

    // and: the address is free again
    assert_eq!(
        wired
            .secrets
            .holder(&EmailAddress::new(ADA).expect("a plain address"))
            .await
            .expect("reading should succeed"),
        None,
        "an address claimed for an account that never came to be would be blocked for good"
    );
    // and: nothing is sent
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}

#[tokio::test]
async fn a_verification_whose_event_cannot_be_kept_leaves_the_code_open() {
    // given: an account signed up with a code sent
    let wired = wiring::wired();
    let id = ada_signed_up(&wired).await;
    let code = wired.last_code_sent_to(ADA);

    // when: it verifies while events cannot be kept
    let refused = wired.refusing_appends().verify(ADA, &code).await;

    // then: the verification fails
    assert!(refused.is_err());
    // and: the address is unverified
    assert!(!is_verified(&wired, &id).await);

    // when: it verifies with the same code again
    wired
        .accounts
        .verify(ADA, &code)
        .await
        .expect("the code is closed only with the event, so it is still open");

    // then: its address is verified
    assert!(is_verified(&wired, &id).await);
}

#[tokio::test]
async fn a_try_is_counted_even_when_the_verification_cannot_be_kept() {
    // given: an account signed up with a code sent
    let wired = wiring::wired();
    ada_signed_up(&wired).await;
    let code = wired.last_code_sent_to(ADA);
    // and: a verification with it that could not be kept
    let _refused = wired.refusing_appends().verify(ADA, &code).await;

    // when: the rest of the tries are spent on wrong guesses
    let right = guessed_wrong(&wired, PendingVerification::MAX_TRIES - 1).await;

    // then: the right code is refused
    assert!(
        matches!(
            wired.accounts.verify(ADA, &right).await,
            Err(AccountsError::CodeExhausted)
        ),
        "a try is counted in a transaction of its own, so a failure after it cannot give it back"
    );
}
