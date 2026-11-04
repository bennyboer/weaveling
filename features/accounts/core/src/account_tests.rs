use eventsourcing::{
    Agent, Aggregate, AggregateContext, AggregateId, Event, EventMetadata, EventName, Version,
};
use time::{Duration, OffsetDateTime};

use crate::account::*;
use email::EmailAddress;
use std::collections::BTreeMap;

use crate::decline::DeclineReason;
use crate::lock::{Lock, LockKind, LockReason};
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
        context: AggregateContext::default(),
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
    // given: an address and a name
    let (email, name) = ada();

    // expect: signing up with them signs them up
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
    // given: a signed up account
    let account = a_signed_up_account();

    // then: it is unverified with its address and moment of signing up
    assert!(!account.is_verified());
    assert_eq!(account.email(), &ada().0);
    assert_eq!(account.signed_up_at(), at(1_000));
}

fn a_sign_up_by_invitation() -> AccountCommand {
    let (email, name) = ada();

    AccountCommand::SignUpByInvitation {
        email,
        name,
        invitation: "invitation_1".to_owned(),
    }
}

#[test]
fn an_account_signed_up_by_invitation_has_its_address_confirmed_from_the_start() {
    // given: an address and a name
    let (email, name) = ada();

    // when: they sign up by invitation
    let signed_up =
        Account::begin(a_sign_up_by_invitation(), &herself()).expect("anyone may sign up");

    // then: the account is signed up and the invitation taken up
    assert_eq!(
        signed_up,
        vec![
            AccountEvent::SignedUpByInvitation { email, name },
            AccountEvent::InvitationTakenUp {
                invitation: "invitation_1".to_owned(),
            },
        ],
        "an invitation is taken up alike whether it signed its address up or not"
    );
    // and: the account has its address confirmed and is no operator
    let account =
        Account::from_first(&signed_up[0], &stamped(1, at(1_000))).expect("signing up gives birth");
    assert!(
        account.is_verified(),
        "the link reached the address, which is all a code would have shown"
    );
    assert_eq!(account.email(), &ada().0);
    assert_eq!(account.name(), &ada().1);
    assert_eq!(account.signed_up_at(), at(1_000));
    assert!(!account.is_operator());
}

#[test]
fn an_account_is_signed_up_once_however_it_came() {
    // given: an account signed up plainly and one signed up by invitation
    let (email, name) = ada();
    let by_invitation_first = Account::from_first(
        &Account::begin(a_sign_up_by_invitation(), &herself()).expect("anyone may sign up")[0],
        &stamped(1, at(1_000)),
    )
    .expect("signing up gives birth");

    // expect: either is refused signing up again either way
    for account in [a_signed_up_account(), by_invitation_first] {
        for again in [
            AccountCommand::SignUp(email.clone(), name.clone()),
            a_sign_up_by_invitation(),
        ] {
            assert_eq!(
                account.decide(again, &herself()),
                Err(AccountError::AlreadySignedUp)
            );
        }
    }
}

#[test]
fn nothing_but_signing_up_can_come_first() {
    // expect: verifying an account never signed up is refused
    assert_eq!(
        Account::begin(AccountCommand::Verify, &herself()),
        Err(AccountError::NotSignedUpYet)
    );
}

#[test]
fn an_account_is_signed_up_once() {
    // given: an address and a name
    let (email, name) = ada();

    // expect: a signed up account refuses signing up with them again
    assert_eq!(
        a_signed_up_account().decide(AccountCommand::SignUp(email, name), &herself()),
        Err(AccountError::AlreadySignedUp)
    );
}

#[test]
fn verifying_happens_once_and_then_changes_nothing() {
    // given: a signed up account
    let mut account = a_signed_up_account();

    // when: it is verified
    let verified = account
        .decide(AccountCommand::Verify, &herself())
        .expect("verifying should succeed");

    // then: its address is verified
    assert_eq!(verified, vec![AccountEvent::EmailVerified]);

    // when: that verifying has happened
    account.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));

    // then: the account is verified
    assert!(account.is_verified());
    // and: verifying again changes nothing
    assert_eq!(
        account.decide(AccountCommand::Verify, &herself()),
        Ok(vec![]),
        "a code entered twice, or a second tab, is no reason to fail"
    );
}

#[test]
fn a_password_reset_is_told_without_the_password() {
    // given: a verified account
    let mut account = a_signed_up_account();
    account.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));

    // expect: resetting its password tells the reset alone
    assert_eq!(
        account.decide(AccountCommand::ResetPassword, &herself()),
        Ok(vec![AccountEvent::PasswordReset])
    );
}

#[test]
fn a_reset_by_a_code_sent_to_the_address_confirms_it_too() {
    // expect: resetting an unverified account's password verifies its address first
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
    // expect: resetting the password of an account never signed up is refused
    assert_eq!(
        Account::begin(AccountCommand::ResetPassword, &herself()),
        Err(AccountError::NotSignedUpYet)
    );
}

#[test]
fn a_reset_ends_every_session_begun_before_it() {
    // given: an account counting a session it began
    let mut account = a_signed_up_account();
    assert!(account.counts_session_begun(at(1_500)));

    // when: its password is reset
    account.apply(&AccountEvent::PasswordReset, &stamped(2, at(2_000)));

    // then: only sessions begun from the reset on count
    assert!(!account.counts_session_begun(at(1_999)));
    assert!(
        account.counts_session_begun(at(2_000)),
        "signing in the moment the password was reset is signing in with the new one"
    );
    assert!(account.counts_session_begun(at(2_001)));
}

#[test]
fn a_snapshot_holds_everything_the_account_is() {
    // given: an account that went through everything
    let mut account = a_signed_up_account();
    account.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));
    account.apply(&AccountEvent::PasswordReset, &stamped(3, at(2_500)));
    account.apply(&AccountEvent::BecameOperator, &stamped(4, at(2_700)));
    account.apply(&a_lock(), &stamped(5, at(2_800)));

    // when: an account is born of its snapshot
    let restored = Account::from_first(&account.snapshot(), &stamped(5, at(3_000)))
        .expect("a snapshot gives birth");

    // then: it is the same account
    assert_eq!(restored, account);
}

#[test]
fn a_snapshot_applied_to_an_account_makes_it_what_the_snapshot_says() {
    // given: an account that went through everything
    let mut further = a_signed_up_account();
    further.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));
    further.apply(&AccountEvent::PasswordReset, &stamped(3, at(2_500)));
    further.apply(&AccountEvent::BecameOperator, &stamped(4, at(2_700)));
    further.apply(&a_lock(), &stamped(5, at(2_800)));
    // and: an account that is only signed up
    let mut behind = a_signed_up_account();

    // when: the snapshot of the first is applied to the second
    behind.apply(&further.snapshot(), &stamped(6, at(3_000)));

    // then: the second is the first
    assert_eq!(behind, further);
}

#[test]
fn verifying_or_a_reset_alone_cannot_begin_an_account() {
    // expect: no event but signing up gives birth to an account
    for event in [
        AccountEvent::EmailVerified,
        AccountEvent::PasswordReset,
        AccountEvent::BecameOperator,
        AccountEvent::StoppedBeingOperator,
        a_lock(),
        an_unlock(),
    ] {
        assert_eq!(Account::from_first(&event, &stamped(1, at(1_000))), None);
    }
}

fn a_verified_account() -> Account {
    let mut account = a_signed_up_account();
    account.apply(&AccountEvent::EmailVerified, &stamped(2, at(2_000)));

    account
}

#[test]
fn an_account_is_no_operator_until_it_becomes_one() {
    // given: a verified account
    let mut account = a_verified_account();
    // and: it is no operator
    assert!(!account.is_operator());

    // when: it becomes one
    let became = account
        .decide(AccountCommand::BecomeOperator, &herself())
        .expect("a verified account may become an operator");
    assert_eq!(became, vec![AccountEvent::BecameOperator]);
    account.apply(&AccountEvent::BecameOperator, &stamped(3, at(3_000)));

    // then: it is an operator
    assert!(account.is_operator());
}

#[test]
fn becoming_an_operator_twice_changes_nothing() {
    // given: an operator
    let mut account = a_verified_account();
    account.apply(&AccountEvent::BecameOperator, &stamped(3, at(3_000)));

    // expect: becoming one again changes nothing
    assert_eq!(
        account.decide(AccountCommand::BecomeOperator, &herself()),
        Ok(vec![])
    );
}

#[test]
fn an_address_not_yet_verified_cannot_become_an_operator() {
    // expect: an unverified account is refused becoming an operator
    assert_eq!(
        a_signed_up_account().decide(AccountCommand::BecomeOperator, &herself()),
        Err(AccountError::NotVerifiedYet),
        "an operator's address must be shown to be theirs, or it could be anyone's"
    );
}

#[test]
fn nobody_becomes_an_operator_before_signing_up() {
    // expect: an account never signed up is refused becoming an operator
    assert_eq!(
        Account::begin(AccountCommand::BecomeOperator, &herself()),
        Err(AccountError::NotSignedUpYet)
    );
}

fn spam() -> LockReason {
    LockReason::new("Sent spam.").expect("a plain reason")
}

fn a_lock() -> AccountEvent {
    AccountEvent::Locked {
        kind: LockKind::Operator,
        reason: spam(),
    }
}

fn an_unlock() -> AccountEvent {
    AccountEvent::Unlocked {
        kind: LockKind::Operator,
    }
}

#[test]
fn an_account_is_locked_for_a_reason_from_the_moment_it_is() {
    // given: a verified account
    let mut account = a_verified_account();
    // and: it is not locked
    assert!(!account.is_locked());

    // when: it is locked for a reason
    let locked = account
        .decide(AccountCommand::Lock(LockKind::Operator, spam()), &herself())
        .expect("an account may be locked");
    assert_eq!(locked, vec![a_lock()]);
    account.apply(&a_lock(), &stamped(3, at(3_000)));

    // then: it is locked
    assert!(account.is_locked());
    // and: its lock holds the reason and the moment it was locked
    assert_eq!(
        account.locks(),
        &BTreeMap::from([(
            LockKind::Operator,
            Lock {
                reason: spam(),
                since: at(3_000),
            }
        )])
    );
}

#[test]
fn a_lock_ends_every_session_begun_before_it() {
    // given: a verified account
    let mut account = a_verified_account();

    // when: it is locked
    account.apply(&a_lock(), &stamped(3, at(3_000)));

    // then: a session begun before the lock no longer counts
    assert!(!account.counts_session_begun(at(2_999)));
}

#[test]
fn locking_twice_changes_nothing_not_even_the_reason() {
    // given: a locked account
    let mut account = a_verified_account();
    account.apply(&a_lock(), &stamped(3, at(3_000)));

    // expect: locking it again for another reason changes nothing
    assert_eq!(
        account.decide(
            AccountCommand::Lock(
                LockKind::Operator,
                LockReason::new("Another reason.").expect("a plain reason")
            ),
            &herself()
        ),
        Ok(vec![])
    );
}

#[test]
fn an_operator_cannot_be_locked() {
    // given: an operator
    let mut account = a_verified_account();
    account.apply(&AccountEvent::BecameOperator, &stamped(3, at(3_000)));

    // expect: locking it is refused
    assert_eq!(
        account.decide(AccountCommand::Lock(LockKind::Operator, spam()), &herself()),
        Err(AccountError::OperatorCannotBeLocked),
        "the role is taken away first, so two operators cannot lock each other out"
    );
}

#[test]
fn unlocking_lifts_the_lock_but_brings_back_no_session() {
    // given: a locked account
    let mut account = a_verified_account();
    account.apply(&a_lock(), &stamped(3, at(3_000)));

    // when: it is unlocked
    let unlocked = account
        .decide(AccountCommand::Unlock(LockKind::Operator), &herself())
        .expect("a locked account may be unlocked");
    assert_eq!(unlocked, vec![an_unlock()]);
    account.apply(&an_unlock(), &stamped(4, at(4_000)));

    // then: it is not locked
    assert!(!account.is_locked());
    // and: a session begun before the lock still does not count
    assert!(
        !account.counts_session_begun(at(2_999)),
        "a session ended by the lock stays ended"
    );
}

#[test]
fn unlocking_an_account_not_locked_changes_nothing() {
    // expect: unlocking an account that is not locked changes nothing
    assert_eq!(
        a_verified_account().decide(AccountCommand::Unlock(LockKind::Operator), &herself()),
        Ok(vec![])
    );
}

#[test]
fn nobody_is_locked_or_unlocked_before_signing_up() {
    // expect: locking or unlocking an account never signed up is refused
    for command in [
        AccountCommand::Lock(LockKind::Operator, spam()),
        AccountCommand::Unlock(LockKind::Operator),
    ] {
        assert_eq!(
            Account::begin(command, &herself()),
            Err(AccountError::NotSignedUpYet)
        );
    }
}

#[test]
fn an_operator_stops_being_one() {
    // given: an operator
    let mut account = a_verified_account();
    account.apply(&AccountEvent::BecameOperator, &stamped(3, at(3_000)));

    // when: it stops being one
    let stopped = account
        .decide(AccountCommand::StopBeingOperator, &herself())
        .expect("an operator may stop being one");
    assert_eq!(stopped, vec![AccountEvent::StoppedBeingOperator]);
    account.apply(&AccountEvent::StoppedBeingOperator, &stamped(4, at(4_000)));

    // then: it is no operator
    assert!(!account.is_operator());
}

#[test]
fn an_account_that_is_no_operator_stops_being_none() {
    // expect: an account that is no operator stopping being one changes nothing
    assert_eq!(
        a_verified_account().decide(AccountCommand::StopBeingOperator, &herself()),
        Ok(vec![]),
        "taking a role away twice, or from two tabs, is no reason to fail"
    );
}

#[test]
fn a_locked_account_cannot_become_an_operator() {
    // given: a locked account
    let mut account = a_verified_account();
    account.apply(&a_lock(), &stamped(3, at(3_000)));

    // expect: becoming an operator is refused
    assert_eq!(
        account.decide(AccountCommand::BecomeOperator, &herself()),
        Err(AccountError::LockedCannotBecomeOperator),
        "or an operator could be made of someone locked, whom no operator can be"
    );
}

#[test]
fn nobody_stops_being_an_operator_before_signing_up() {
    // expect: an account never signed up is refused stopping being an operator
    assert_eq!(
        Account::begin(AccountCommand::StopBeingOperator, &herself()),
        Err(AccountError::NotSignedUpYet)
    );
}

#[test]
fn an_account_declines_an_invitation_to_its_address_and_stays_as_it_was() {
    // given: a signed up account
    let account = a_signed_up_account();
    let declining = AccountCommand::DeclineInvitation {
        invitation: "invitation_1".to_owned(),
        reason: DeclineReason::AddressHasAccount,
    };

    // when: it declines an invitation
    let declined = account
        .decide(declining.clone(), &Agent::System)
        .expect("an account declines what it does not need");
    let mut after = account.clone();
    after.apply(&declined[0], &stamped(2, at(2_000)));

    // then: the decline is told with its reason
    assert_eq!(
        declined,
        vec![AccountEvent::InvitationDeclined {
            invitation: "invitation_1".to_owned(),
            reason: DeclineReason::AddressHasAccount,
        }]
    );
    // and: the account stays as it was
    assert_eq!(after, account);
    // and: no account is begun or born of declining
    assert_eq!(
        Account::begin(declining, &Agent::System),
        Err(AccountError::NotSignedUpYet)
    );
    assert_eq!(
        Account::from_first(&declined[0], &stamped(1, at(1_000))),
        None
    );
}

#[test]
fn an_account_takes_an_invitation_up_and_stays_as_it_was() {
    // given: a signed up account
    let account = a_signed_up_account();
    let taking_up = AccountCommand::TakeUpInvitation {
        invitation: "invitation_1".to_owned(),
    };

    // when: it takes up an invitation
    let taken_up = account
        .decide(taking_up.clone(), &Agent::System)
        .expect("an account takes up what was sent to its address");
    let mut after = account.clone();
    after.apply(&taken_up[0], &stamped(2, at(2_000)));

    // then: the taking up is told
    assert_eq!(
        taken_up,
        vec![AccountEvent::InvitationTakenUp {
            invitation: "invitation_1".to_owned(),
        }]
    );
    // and: under its name
    assert_eq!(
        taken_up[0].name(),
        EventName::of("INVITATION_TAKEN_UP"),
        "the name is how the event is kept and told"
    );
    // and: the account stays as it was
    assert_eq!(after, account);
    // and: no account is begun or born of taking up
    assert_eq!(
        Account::begin(taking_up, &Agent::System),
        Err(AccountError::NotSignedUpYet)
    );
    assert_eq!(
        Account::from_first(&taken_up[0], &stamped(1, at(1_000))),
        None
    );
}
