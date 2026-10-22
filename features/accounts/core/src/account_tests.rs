use eventsourcing::{Agent, Aggregate, AggregateId, EventMetadata, Version};
use time::{Duration, OffsetDateTime};

use crate::account::*;
use crate::email::EmailAddress;
use crate::name::AccountName;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn herself() -> Agent {
    Agent::Account("account_1".into())
}

fn stamped(version: u64, occurred_at: OffsetDateTime) -> EventMetadata {
    EventMetadata {
        aggregate: AggregateId::from("account_1"),
        kind: KIND,
        version: Version::of(version),
        agent: herself(),
        occurred_at,
        is_snapshot: false,
    }
}

fn ada() -> (EmailAddress, AccountName) {
    (
        EmailAddress::new("ada@example.com").expect("a plain address is fine"),
        AccountName::new("Ada").expect("a plain name is fine"),
    )
}

fn a_signed_up_account() -> Account {
    let (email, name) = ada();

    Account::from_first(
        &AccountEvent::SignedUp { email, name },
        &stamped(1, at(1_000)),
    )
    .expect("signing up gives birth")
}

#[test]
fn an_account_is_signed_up_with_an_address_and_a_name() {
    let (email, name) = ada();

    assert_eq!(
        Account::begin(
            AccountCommand::SignUp(email.clone(), name.clone()),
            &herself()
        ),
        Ok(vec![AccountEvent::SignedUp { email, name }])
    );
}

#[test]
fn a_signed_up_account_is_not_verified_yet() {
    let account = a_signed_up_account();

    assert!(!account.is_verified());
    assert_eq!(account.email(), &ada().0);
    assert_eq!(account.signed_up_at(), at(1_000));
}

#[test]
fn nothing_but_signing_up_can_come_first() {
    assert_eq!(
        Account::begin(AccountCommand::Verify, &herself()),
        Err(AccountError::NotSignedUpYet)
    );
}

#[test]
fn an_account_is_signed_up_once() {
    let (email, name) = ada();

    assert_eq!(
        a_signed_up_account().decide(AccountCommand::SignUp(email, name), &herself()),
        Err(AccountError::AlreadySignedUp)
    );
}

#[test]
fn verifying_happens_once_and_then_changes_nothing() {
    let mut account = a_signed_up_account();

    let verified = account
        .decide(AccountCommand::Verify, &herself())
        .expect("verifying should succeed");
    assert_eq!(verified, vec![AccountEvent::EmailVerified]);

    account.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));
    assert!(account.is_verified());
    assert_eq!(
        account.decide(AccountCommand::Verify, &herself()),
        Ok(vec![]),
        "a code entered twice, or a second tab, is no reason to fail"
    );
}

#[test]
fn a_password_reset_is_told_without_the_password() {
    let mut account = a_signed_up_account();
    account.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));

    assert_eq!(
        account.decide(AccountCommand::ResetPassword, &herself()),
        Ok(vec![AccountEvent::PasswordReset])
    );
}

#[test]
fn a_reset_by_a_code_sent_to_the_address_confirms_it_too() {
    assert_eq!(
        a_signed_up_account().decide(AccountCommand::ResetPassword, &herself()),
        Ok(vec![
            AccountEvent::EmailVerified,
            AccountEvent::PasswordReset
        ]),
        "the code reached the address, which is all confirming it asks"
    );
}

#[test]
fn no_password_is_reset_before_signing_up() {
    assert_eq!(
        Account::begin(AccountCommand::ResetPassword, &herself()),
        Err(AccountError::NotSignedUpYet)
    );
}

#[test]
fn a_reset_ends_every_session_begun_before_it() {
    let mut account = a_signed_up_account();
    assert!(account.counts_session_begun(at(1_500)));

    account.apply(&AccountEvent::PasswordReset, &stamped(2, at(2_000)));

    assert!(!account.counts_session_begun(at(1_999)));
    assert!(
        account.counts_session_begun(at(2_000)),
        "signing in the moment the password was reset is signing in with the new one"
    );
    assert!(account.counts_session_begun(at(2_001)));
}

#[test]
fn a_snapshot_holds_everything_the_account_is() {
    let mut account = a_signed_up_account();
    account.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));
    account.apply(&AccountEvent::PasswordReset, &stamped(3, at(2_500)));

    let restored = Account::from_first(&account.snapshot(), &stamped(3, at(3_000)))
        .expect("a snapshot gives birth");

    assert_eq!(restored, account);
}

#[test]
fn verifying_or_a_reset_alone_cannot_begin_an_account() {
    for event in [AccountEvent::EmailVerified, AccountEvent::PasswordReset] {
        assert_eq!(Account::from_first(&event, &stamped(1, at(1_000))), None);
    }
}
