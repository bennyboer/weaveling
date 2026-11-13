use eventsourcing::{
    Agent, Aggregate, AggregateContext, AggregateId, Event, EventMetadata, EventName, Version,
};
use time::OffsetDateTime;

use crate::account::*;
use email::EmailAddress;
use std::collections::BTreeMap;

use crate::decline::DeclineReason;
use crate::deletion::{DeletionAskedBy, PendingDeletion};
use crate::key::{KeyId, KeyName, KeyRole};
use crate::lock::{Lock, LockKind, LockReason};
use crate::name::AccountName;
use crate::picture::PictureId;
use crate::second_factor::{PreferredWay, SecondFactor, SecondStepWay};

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
        &stamped(1, clock::at(1_000)),
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
    assert_eq!(account.signed_up_at(), clock::at(1_000));
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
    let account = Account::from_first(&signed_up[0], &stamped(1, clock::at(1_000)))
        .expect("signing up gives birth");
    assert!(
        account.is_verified(),
        "the link reached the address, which is all a code would have shown"
    );
    assert_eq!(account.email(), &ada().0);
    assert_eq!(account.name(), &ada().1);
    assert_eq!(account.signed_up_at(), clock::at(1_000));
    assert!(!account.is_operator());
}

#[test]
fn an_account_is_signed_up_once_however_it_came() {
    // given: an account signed up plainly and one signed up by invitation
    let (email, name) = ada();
    let by_invitation_first = Account::from_first(
        &Account::begin(a_sign_up_by_invitation(), &herself()).expect("anyone may sign up")[0],
        &stamped(1, clock::at(1_000)),
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
    account.apply(&AccountEvent::EmailVerified, &stamped(2, clock::at(2_000)));

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
    account.apply(&AccountEvent::EmailVerified, &stamped(2, clock::at(2_000)));

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
    assert!(account.counts_session_begun(clock::at(1_500)));

    // when: its password is reset
    account.apply(&AccountEvent::PasswordReset, &stamped(2, clock::at(2_000)));

    // then: only sessions begun from the reset on count
    assert!(!account.counts_session_begun(clock::at(1_999)));
    assert!(
        account.counts_session_begun(clock::at(2_000)),
        "signing in the moment the password was reset is signing in with the new one"
    );
    assert!(account.counts_session_begun(clock::at(2_001)));
}

#[test]
fn a_snapshot_holds_everything_the_account_is() {
    // given: an account that went through everything
    let mut account = a_signed_up_account();
    account.apply(&AccountEvent::EmailVerified, &stamped(2, clock::at(2_000)));
    account.apply(&AccountEvent::PasswordReset, &stamped(3, clock::at(2_500)));
    account.apply(&AccountEvent::BecameOperator, &stamped(4, clock::at(2_700)));
    account.apply(&a_lock(), &stamped(5, clock::at(2_800)));
    account.apply(&AccountEvent::TotpAdded, &stamped(6, clock::at(2_850)));
    account.apply(
        &AccountEvent::RecoveryCodesDrawn,
        &stamped(7, clock::at(2_850)),
    );
    account.apply(
        &AccountEvent::RecoveryCodeUsed,
        &stamped(8, clock::at(2_900)),
    );
    account.apply(
        &a_key_added("YubiKey blue", KeyRole::SecondFactor),
        &stamped(9, clock::at(2_950)),
    );
    account.apply(
        &AccountEvent::PreferredWayChosen {
            way: PreferredWay::Chosen(SecondStepWay::Totp),
        },
        &stamped(10, clock::at(2_960)),
    );
    account.apply(
        &deletion_asked_by(DeletionAskedBy::Operator),
        &stamped(11, clock::at(2_970)),
    );

    // when: an account is born of its snapshot
    let restored = Account::from_first(&account.snapshot(), &stamped(9, clock::at(3_000)))
        .expect("a snapshot gives birth");

    // then: it is the same account
    assert_eq!(restored, account);
}

#[test]
fn a_snapshot_applied_to_an_account_makes_it_what_the_snapshot_says() {
    // given: an account that went through everything
    let mut further = a_signed_up_account();
    further.apply(&AccountEvent::EmailVerified, &stamped(2, clock::at(2_000)));
    further.apply(&AccountEvent::PasswordReset, &stamped(3, clock::at(2_500)));
    further.apply(&AccountEvent::BecameOperator, &stamped(4, clock::at(2_700)));
    further.apply(&a_lock(), &stamped(5, clock::at(2_800)));
    further.apply(&AccountEvent::TotpAdded, &stamped(6, clock::at(2_850)));
    further.apply(
        &AccountEvent::RecoveryCodesDrawn,
        &stamped(7, clock::at(2_850)),
    );
    further.apply(
        &AccountEvent::RecoveryCodeUsed,
        &stamped(8, clock::at(2_900)),
    );
    further.apply(
        &a_key_added("MacBook", KeyRole::Primary),
        &stamped(9, clock::at(2_950)),
    );
    further.apply(
        &deletion_asked_by(DeletionAskedBy::Holder),
        &stamped(10, clock::at(2_970)),
    );
    // and: an account that is only signed up
    let mut behind = a_signed_up_account();

    // when: the snapshot of the first is applied to the second
    behind.apply(&further.snapshot(), &stamped(9, clock::at(3_000)));

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
        assert_eq!(
            Account::from_first(&event, &stamped(1, clock::at(1_000))),
            None
        );
    }
}

fn a_verified_account() -> Account {
    let mut account = a_signed_up_account();
    account.apply(&AccountEvent::EmailVerified, &stamped(2, clock::at(2_000)));

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
    account.apply(&AccountEvent::BecameOperator, &stamped(3, clock::at(3_000)));

    // then: it is an operator
    assert!(account.is_operator());
}

#[test]
fn becoming_an_operator_twice_changes_nothing() {
    // given: an operator
    let mut account = a_verified_account();
    account.apply(&AccountEvent::BecameOperator, &stamped(3, clock::at(3_000)));

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
    account.apply(&a_lock(), &stamped(3, clock::at(3_000)));

    // then: it is locked
    assert!(account.is_locked());
    // and: its lock holds the reason and the moment it was locked
    assert_eq!(
        account.locks(),
        &BTreeMap::from([(
            LockKind::Operator,
            Lock {
                reason: spam(),
                since: clock::at(3_000),
            }
        )])
    );
}

#[test]
fn a_lock_ends_every_session_begun_before_it() {
    // given: a verified account
    let mut account = a_verified_account();

    // when: it is locked
    account.apply(&a_lock(), &stamped(3, clock::at(3_000)));

    // then: a session begun before the lock no longer counts
    assert!(!account.counts_session_begun(clock::at(2_999)));
}

#[test]
fn locking_twice_changes_nothing_not_even_the_reason() {
    // given: a locked account
    let mut account = a_verified_account();
    account.apply(&a_lock(), &stamped(3, clock::at(3_000)));

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
    account.apply(&AccountEvent::BecameOperator, &stamped(3, clock::at(3_000)));

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
    account.apply(&a_lock(), &stamped(3, clock::at(3_000)));

    // when: it is unlocked
    let unlocked = account
        .decide(AccountCommand::Unlock(LockKind::Operator), &herself())
        .expect("a locked account may be unlocked");
    assert_eq!(unlocked, vec![an_unlock()]);
    account.apply(&an_unlock(), &stamped(4, clock::at(4_000)));

    // then: it is not locked
    assert!(!account.is_locked());
    // and: a session begun before the lock still does not count
    assert!(
        !account.counts_session_begun(clock::at(2_999)),
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
    account.apply(&AccountEvent::BecameOperator, &stamped(3, clock::at(3_000)));

    // when: it stops being one
    let stopped = account
        .decide(AccountCommand::StopBeingOperator, &herself())
        .expect("an operator may stop being one");
    assert_eq!(stopped, vec![AccountEvent::StoppedBeingOperator]);
    account.apply(
        &AccountEvent::StoppedBeingOperator,
        &stamped(4, clock::at(4_000)),
    );

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
    account.apply(&a_lock(), &stamped(3, clock::at(3_000)));

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
    after.apply(&declined[0], &stamped(2, clock::at(2_000)));

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
        Account::from_first(&declined[0], &stamped(1, clock::at(1_000))),
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
    after.apply(&taken_up[0], &stamped(2, clock::at(2_000)));

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
        Account::from_first(&taken_up[0], &stamped(1, clock::at(1_000))),
        None
    );
}

fn with_a_second_factor_since(at: i64) -> Account {
    let mut account = a_verified_account();
    let stamp = stamped(3, clock::at(at));
    for event in account
        .decide(AccountCommand::AddTotp, &herself())
        .expect("a second factor may be added")
    {
        account.apply(&event, &stamp);
    }

    account
}

#[test]
fn a_second_factor_is_added_with_ten_recovery_codes() {
    // given: a verified account without a second factor
    let account = a_verified_account();
    assert_eq!(account.second_factor(), None);

    // when: a second factor is added
    let added = account
        .decide(AccountCommand::AddTotp, &herself())
        .expect("a second factor may be added");

    // then: it is added, and its recovery codes drawn
    assert_eq!(
        added,
        vec![AccountEvent::TotpAdded, AccountEvent::RecoveryCodesDrawn]
    );
    // and: the account holds it from that moment, with ten codes left
    assert_eq!(
        with_a_second_factor_since(3_000).second_factor(),
        Some(SecondFactor {
            since: clock::at(3_000),
            recovery_codes_left: 10,
            answered_by_totp: true,
            answered_by_key: false,
        })
    );
}

#[test]
fn adding_or_removing_a_second_factor_ends_every_session_begun_before() {
    // given: an account with a second factor added
    let mut account = with_a_second_factor_since(3_000);

    // then: a session begun before it no longer counts
    assert!(!account.counts_session_begun(clock::at(2_999)));
    assert!(account.counts_session_begun(clock::at(3_000)));
    // when: the second factor is removed
    account.apply(&AccountEvent::TotpRemoved, &stamped(5, clock::at(4_000)));
    // then: a session begun before that no longer counts either
    assert!(!account.counts_session_begun(clock::at(3_999)));
}

#[test]
fn a_second_factor_added_again_replaces_the_one_before() {
    // given: an account with a second factor and a recovery code used
    let mut account = with_a_second_factor_since(3_000);
    account.apply(
        &AccountEvent::RecoveryCodeUsed,
        &stamped(4, clock::at(3_500)),
    );

    // when: a second factor is added again
    for event in account
        .decide(AccountCommand::AddTotp, &herself())
        .expect("a second factor may be set up anew")
    {
        account.apply(&event, &stamped(5, clock::at(4_000)));
    }

    // then: it holds the new one, with ten codes again
    assert_eq!(
        account.second_factor(),
        Some(SecondFactor {
            since: clock::at(4_000),
            recovery_codes_left: 10,
            answered_by_totp: true,
            answered_by_key: false,
        })
    );
}

#[test]
fn each_recovery_code_used_leaves_one_fewer() {
    // given: an account with a second factor
    let mut account = with_a_second_factor_since(3_000);

    // when: a recovery code is used
    let used = account
        .decide(AccountCommand::UseRecoveryCode, &herself())
        .expect("a code is left");
    assert_eq!(used, vec![AccountEvent::RecoveryCodeUsed]);
    account.apply(
        &AccountEvent::RecoveryCodeUsed,
        &stamped(4, clock::at(3_500)),
    );

    // then: nine are left
    assert_eq!(
        account
            .second_factor()
            .map(|factor| factor.recovery_codes_left),
        Some(9)
    );
    // when: the other nine are used
    for version in 5..14 {
        account.apply(
            &AccountEvent::RecoveryCodeUsed,
            &stamped(version, clock::at(3_600)),
        );
    }
    // then: none is left, and using one is refused
    assert_eq!(
        account.decide(AccountCommand::UseRecoveryCode, &herself()),
        Err(AccountError::NoRecoveryCodeLeft)
    );
    // when: codes are drawn anew
    assert_eq!(
        account.decide(AccountCommand::DrawRecoveryCodes, &herself()),
        Ok(vec![AccountEvent::RecoveryCodesDrawn])
    );
    account.apply(
        &AccountEvent::RecoveryCodesDrawn,
        &stamped(14, clock::at(3_700)),
    );
    // then: ten are left
    assert_eq!(
        account
            .second_factor()
            .map(|factor| factor.recovery_codes_left),
        Some(10)
    );
}

#[test]
fn recovery_codes_belong_to_a_second_factor() {
    // given: an account without a second factor
    let account = a_verified_account();

    // expect: drawing or using recovery codes is refused
    for command in [
        AccountCommand::DrawRecoveryCodes,
        AccountCommand::UseRecoveryCode,
    ] {
        assert_eq!(
            account.decide(command, &herself()),
            Err(AccountError::NoSecondFactor)
        );
    }
}

#[test]
fn removing_a_second_factor_takes_its_recovery_codes_along() {
    // given: an account with a second factor
    let account = with_a_second_factor_since(3_000);

    // when: it is removed
    let removed = account
        .decide(AccountCommand::RemoveTotp, &herself())
        .expect("a second factor may be removed");
    assert_eq!(removed, vec![AccountEvent::TotpRemoved]);
    let mut account = account;
    account.apply(&AccountEvent::TotpRemoved, &stamped(4, clock::at(4_000)));

    // then: the account has none
    assert_eq!(account.second_factor(), None);
    // and: removing it again changes nothing
    assert_eq!(
        account.decide(AccountCommand::RemoveTotp, &herself()),
        Ok(vec![])
    );
}

#[test]
fn nothing_about_a_second_factor_happens_before_signing_up() {
    // expect: every second factor command is refused before signing up
    for command in [
        AccountCommand::AddTotp,
        AccountCommand::DrawRecoveryCodes,
        AccountCommand::UseRecoveryCode,
        AccountCommand::RemoveTotp,
    ] {
        assert_eq!(
            Account::begin(command, &herself()),
            Err(AccountError::NotSignedUpYet)
        );
    }
}

fn a_key() -> KeyId {
    KeyId::generate(clock::at(5_000))
}

fn named(name: &str) -> KeyName {
    KeyName::new(name).expect("a plain key name")
}

fn a_key_added(name: &str, role: KeyRole) -> AccountEvent {
    AccountEvent::KeyAdded {
        key: a_key(),
        name: named(name),
        role,
    }
}

fn adding(
    account: &mut Account,
    key: KeyId,
    name: &str,
    role: KeyRole,
    at: i64,
) -> Vec<AccountEvent> {
    let added = account
        .decide(
            AccountCommand::AddKey {
                key,
                name: named(name),
                role,
            },
            &herself(),
        )
        .expect("the key may be added");
    for event in &added {
        account.apply(event, &stamped(10, clock::at(at)));
    }

    added
}

#[test]
fn a_second_step_key_as_the_first_second_factor_comes_with_ten_recovery_codes() {
    // given: a verified account without a second factor
    let mut account = a_verified_account();
    let key = a_key();

    // when: a key is added to answer the second step
    let added = adding(
        &mut account,
        key,
        "YubiKey blue",
        KeyRole::SecondFactor,
        3_000,
    );

    // then: it is added, and recovery codes drawn
    assert_eq!(
        added,
        vec![
            AccountEvent::KeyAdded {
                key,
                name: named("YubiKey blue"),
                role: KeyRole::SecondFactor,
            },
            AccountEvent::RecoveryCodesDrawn,
        ]
    );
    // and: the account has a second step from that moment, with ten codes
    assert_eq!(
        account.second_factor(),
        Some(SecondFactor {
            since: clock::at(3_000),
            recovery_codes_left: 10,
            answered_by_totp: false,
            answered_by_key: true,
        })
    );
    // and: a session begun before it no longer counts
    assert!(!account.counts_session_begun(clock::at(2_999)));
}

#[test]
fn a_second_step_key_beside_totp_draws_no_codes_and_the_second_step_keeps_its_moment() {
    // given: an account with TOTP and a recovery code used
    let mut account = with_a_second_factor_since(3_000);
    account.apply(
        &AccountEvent::RecoveryCodeUsed,
        &stamped(4, clock::at(3_500)),
    );

    // when: a key is added to answer the second step as well
    let added = adding(
        &mut account,
        a_key(),
        "YubiKey blue",
        KeyRole::SecondFactor,
        4_000,
    );

    // then: no codes are drawn
    assert_eq!(added.len(), 1);
    // and: the second step is still the one since TOTP, its codes as they were
    assert_eq!(
        account.second_factor(),
        Some(SecondFactor {
            since: clock::at(3_000),
            recovery_codes_left: 9,
            answered_by_totp: true,
            answered_by_key: true,
        })
    );
}

#[test]
fn a_primary_key_is_no_second_factor_and_ends_no_session() {
    // given: a verified account without a second factor
    let mut account = a_verified_account();

    // when: a passkey signing in alone is added
    let added = adding(&mut account, a_key(), "MacBook", KeyRole::Primary, 3_000);

    // then: no codes are drawn, and there is no second step
    assert_eq!(added.len(), 1);
    assert_eq!(account.second_factor(), None);
    // and: a session begun before it still counts
    assert!(account.counts_session_begun(clock::at(2_000)));
    // and: the account holds the key
    assert_eq!(account.keys().len(), 1);
}

#[test]
fn no_two_keys_of_an_account_share_an_id_or_a_name() {
    // given: an account holding a key
    let mut account = a_verified_account();
    let key = a_key();
    adding(
        &mut account,
        key,
        "YubiKey blue",
        KeyRole::SecondFactor,
        3_000,
    );

    // expect: one more by the same id is refused
    assert_eq!(
        account.decide(
            AccountCommand::AddKey {
                key,
                name: named("YubiKey green"),
                role: KeyRole::SecondFactor,
            },
            &herself()
        ),
        Err(AccountError::KeyAlreadyAdded)
    );
    // and: one more by the same name is refused
    assert_eq!(
        account.decide(
            AccountCommand::AddKey {
                key: KeyId::generate(clock::at(5_001)),
                name: named("YubiKey blue"),
                role: KeyRole::Primary,
            },
            &herself()
        ),
        Err(AccountError::KeyNameTaken)
    );
}

#[test]
fn an_account_holds_at_most_twenty_keys() {
    // given: an account holding twenty keys
    let mut account = a_verified_account();
    for number in 0..Account::MOST_KEYS {
        adding(
            &mut account,
            KeyId::generate(clock::at(5_000)),
            &format!("Key {number}"),
            KeyRole::Primary,
            3_000,
        );
    }

    // expect: one more is refused
    assert_eq!(
        account.decide(
            AccountCommand::AddKey {
                key: KeyId::generate(clock::at(5_000)),
                name: named("One too many"),
                role: KeyRole::Primary,
            },
            &herself()
        ),
        Err(AccountError::TooManyKeys)
    );
}

#[test]
fn a_key_is_renamed_to_a_name_no_other_key_has() {
    // given: an account holding two keys
    let mut account = a_verified_account();
    let blue = a_key();
    let green = KeyId::generate(clock::at(5_001));
    adding(
        &mut account,
        blue,
        "YubiKey blue",
        KeyRole::SecondFactor,
        3_000,
    );
    adding(
        &mut account,
        green,
        "YubiKey green",
        KeyRole::SecondFactor,
        3_000,
    );
    let rename = |key, name: &str| AccountCommand::RenameKey {
        key,
        name: named(name),
    };

    // expect: renaming one to the other's name is refused
    assert_eq!(
        account.decide(rename(blue, "YubiKey green"), &herself()),
        Err(AccountError::KeyNameTaken)
    );
    // and: renaming one to its own name changes nothing
    assert_eq!(
        account.decide(rename(blue, "YubiKey blue"), &herself()),
        Ok(vec![])
    );
    // and: renaming a key the account does not hold is refused
    assert_eq!(
        account.decide(
            rename(KeyId::generate(clock::at(5_002)), "Lost"),
            &herself()
        ),
        Err(AccountError::NoSuchKey)
    );
    // when: one is renamed to a free name
    let renamed = account
        .decide(rename(blue, "Keyring"), &herself())
        .expect("a free name");
    for event in &renamed {
        account.apply(event, &stamped(11, clock::at(4_000)));
    }
    // then: it holds that name
    assert_eq!(account.keys()[&blue].name, named("Keyring"));
}

#[test]
fn removing_the_last_second_factor_drops_the_codes_and_ends_every_session() {
    // given: an account whose only second factor is a key
    let mut account = a_verified_account();
    let key = a_key();
    adding(
        &mut account,
        key,
        "YubiKey blue",
        KeyRole::SecondFactor,
        3_000,
    );

    // when: the key is removed
    let removed = account
        .decide(AccountCommand::RemoveKey { key }, &herself())
        .expect("its key");
    assert_eq!(removed, vec![AccountEvent::KeyRemoved { key }]);
    account.apply(&removed[0], &stamped(11, clock::at(4_000)));

    // then: there is no second step, nor codes for one
    assert_eq!(account.second_factor(), None);
    assert!(account.keys().is_empty());
    assert_eq!(
        account.decide(AccountCommand::UseRecoveryCode, &herself()),
        Err(AccountError::NoSecondFactor)
    );
    // and: a session begun before no longer counts
    assert!(!account.counts_session_begun(clock::at(3_999)));
    // and: removing it again changes nothing
    assert_eq!(
        account.decide(AccountCommand::RemoveKey { key }, &herself()),
        Ok(vec![])
    );
}

#[test]
fn turning_totp_off_beside_a_second_step_key_keeps_the_codes() {
    // given: an account with TOTP, a code used, and a second-step key added later
    let mut account = with_a_second_factor_since(3_000);
    account.apply(
        &AccountEvent::RecoveryCodeUsed,
        &stamped(4, clock::at(3_500)),
    );
    adding(
        &mut account,
        a_key(),
        "YubiKey blue",
        KeyRole::SecondFactor,
        4_000,
    );

    // when: TOTP is turned off
    account.apply(&AccountEvent::TotpRemoved, &stamped(11, clock::at(5_000)));

    // then: the key answers the second step since it was added, the codes kept
    assert_eq!(
        account.second_factor(),
        Some(SecondFactor {
            since: clock::at(4_000),
            recovery_codes_left: 9,
            answered_by_totp: false,
            answered_by_key: true,
        })
    );
}

#[test]
fn the_way_asked_first_is_the_one_chosen_else_the_one_used_last_else_a_key() {
    // given: an account with TOTP and a second-step key, choosing nothing
    let mut account = with_a_second_factor_since(3_000);
    adding(
        &mut account,
        a_key(),
        "YubiKey blue",
        KeyRole::SecondFactor,
        4_000,
    );

    // expect: the way used last is asked first, and a key where none was used yet
    assert_eq!(account.preferred_way(), PreferredWay::LastUsed);
    assert_eq!(
        account.asked_way(Some(SecondStepWay::Totp)),
        Some(SecondStepWay::Totp)
    );
    assert_eq!(account.asked_way(None), Some(SecondStepWay::Key));
    // when: the app is chosen
    let chosen = account
        .decide(
            AccountCommand::ChoosePreferredWay(PreferredWay::Chosen(SecondStepWay::Totp)),
            &herself(),
        )
        .expect("a way may be chosen");
    for event in &chosen {
        account.apply(event, &stamped(11, clock::at(5_000)));
    }
    // then: the app is asked first, whatever answered last
    assert_eq!(
        account.asked_way(Some(SecondStepWay::Key)),
        Some(SecondStepWay::Totp)
    );
    // and: choosing it again changes nothing
    assert_eq!(
        account.decide(
            AccountCommand::ChoosePreferredWay(PreferredWay::Chosen(SecondStepWay::Totp)),
            &herself()
        ),
        Ok(vec![])
    );
}

#[test]
fn a_way_that_no_longer_answers_is_not_asked_first() {
    // given: an account that chose keys first, with TOTP alone answering
    let mut account = with_a_second_factor_since(3_000);
    account.apply(
        &AccountEvent::PreferredWayChosen {
            way: PreferredWay::Chosen(SecondStepWay::Key),
        },
        &stamped(11, clock::at(4_000)),
    );

    // expect: the app is asked first, keys answering nothing
    assert_eq!(
        account.asked_way(Some(SecondStepWay::Key)),
        Some(SecondStepWay::Totp)
    );
    // and: an account without a second step is asked nothing first
    assert_eq!(a_verified_account().asked_way(None), None);
}

#[test]
fn a_passkey_demoted_as_the_first_second_factor_comes_with_recovery_codes_and_ends_every_session() {
    // given: an account whose only key is a passkey
    let mut account = a_verified_account();
    let key = a_key();
    adding(&mut account, key, "MacBook", KeyRole::Primary, 3_000);

    // when: the passkey is demoted to answer the second step
    let demoted = account
        .decide(AccountCommand::DemoteKey { key }, &herself())
        .expect("its passkey");
    for event in &demoted {
        account.apply(event, &stamped(11, clock::at(4_000)));
    }

    // then: it answers the second step, with ten recovery codes, as the first second factor
    assert_eq!(
        demoted,
        vec![
            AccountEvent::KeyDemoted { key },
            AccountEvent::RecoveryCodesDrawn
        ]
    );
    assert_eq!(account.keys()[&key].role, KeyRole::SecondFactor);
    assert_eq!(
        account
            .second_factor()
            .map(|factor| factor.recovery_codes_left),
        Some(10)
    );
    // and: a session begun before no longer counts
    assert!(!account.counts_session_begun(clock::at(3_999)));
    // and: demoting it again changes nothing, and a key not held is refused
    assert_eq!(
        account.decide(AccountCommand::DemoteKey { key }, &herself()),
        Ok(vec![])
    );
    assert_eq!(
        account.decide(
            AccountCommand::DemoteKey {
                key: KeyId::generate(clock::at(5_002))
            },
            &herself()
        ),
        Err(AccountError::NoSuchKey)
    );
}

#[test]
fn a_passkey_demoted_beside_a_second_factor_draws_no_codes() {
    // given: an account with TOTP and a passkey
    let mut account = with_a_second_factor_since(3_000);
    let key = a_key();
    adding(&mut account, key, "MacBook", KeyRole::Primary, 4_000);

    // when: the passkey is demoted
    let demoted = account
        .decide(AccountCommand::DemoteKey { key }, &herself())
        .expect("its passkey");

    // then: only its role changes
    assert_eq!(demoted, vec![AccountEvent::KeyDemoted { key }]);
}

fn deletion_asked_by(by: DeletionAskedBy) -> AccountEvent {
    AccountEvent::DeletionAsked {
        until: clock::at(9_000),
        by,
    }
}

fn asked_to_be_deleted_by(by: DeletionAskedBy) -> Account {
    let mut account = a_verified_account();
    account.apply(&deletion_asked_by(by), &stamped(3, clock::at(3_000)));

    account
}

#[test]
fn an_account_asked_to_be_deleted_is_kept_until_then_its_sessions_ended_and_signing_in_kept_from_it()
 {
    // given: a verified account
    let account = a_verified_account();

    // when: its holder asks for it to be deleted
    let asked = account
        .decide(
            AccountCommand::AskDeletion {
                until: clock::at(9_000),
                by: DeletionAskedBy::Holder,
            },
            &herself(),
        )
        .expect("an account may ask to be deleted");

    // then: the deletion is asked, until when it is kept and by whom
    assert_eq!(asked, vec![deletion_asked_by(DeletionAskedBy::Holder)]);
    // when: it is applied
    let account = asked_to_be_deleted_by(DeletionAskedBy::Holder);
    // then: the account is kept until then, locked, and no session begun before counts
    assert_eq!(
        account.pending_deletion(),
        Some(PendingDeletion {
            until: clock::at(9_000),
            asked_by: DeletionAskedBy::Holder,
        })
    );
    assert!(account.is_locked());
    assert!(!account.counts_session_begun(clock::at(2_999)));
    // and: asking again changes nothing, not even the day
    assert_eq!(
        account.decide(
            AccountCommand::AskDeletion {
                until: clock::at(99_000),
                by: DeletionAskedBy::Operator,
            },
            &herself()
        ),
        Ok(vec![])
    );
}

#[test]
fn an_operator_asking_to_be_deleted_stops_being_one_in_the_same_step() {
    // given: an operator
    let mut account = a_verified_account();
    account.apply(&AccountEvent::BecameOperator, &stamped(3, clock::at(3_000)));

    // when: they ask for their account to be deleted
    let asked = account
        .decide(
            AccountCommand::AskDeletion {
                until: clock::at(9_000),
                by: DeletionAskedBy::Holder,
            },
            &herself(),
        )
        .expect("an operator may ask to be deleted");

    // then: they stop being an operator, then the deletion is asked
    assert_eq!(
        asked,
        vec![
            AccountEvent::StoppedBeingOperator,
            deletion_asked_by(DeletionAskedBy::Holder),
        ]
    );
}

#[test]
fn the_holder_keeps_an_account_they_asked_to_delete_but_not_one_the_operator_marked() {
    // given: an account its holder asked to delete, and one the operator marked
    let asked_by_holder = asked_to_be_deleted_by(DeletionAskedBy::Holder);
    let marked = asked_to_be_deleted_by(DeletionAskedBy::Operator);
    let calling_off = |by| AccountCommand::CallOffDeletion { by };

    // expect: the holder calls off their own asking
    assert_eq!(
        asked_by_holder.decide(calling_off(DeletionAskedBy::Holder), &herself()),
        Ok(vec![AccountEvent::DeletionCalledOff])
    );
    // and: not the operator's
    assert_eq!(
        marked.decide(calling_off(DeletionAskedBy::Holder), &herself()),
        Err(AccountError::DeletionAskedByTheOperator)
    );
    // and: the operator calls off either
    for account in [&asked_by_holder, &marked] {
        assert_eq!(
            account.decide(calling_off(DeletionAskedBy::Operator), &herself()),
            Ok(vec![AccountEvent::DeletionCalledOff])
        );
    }
    // and: calling off where nothing is asked changes nothing
    assert_eq!(
        a_verified_account().decide(calling_off(DeletionAskedBy::Holder), &herself()),
        Ok(vec![])
    );
}

#[test]
fn an_account_kept_after_all_is_no_longer_locked_nor_asked_to_be_deleted() {
    // given: an account asked to be deleted
    let mut account = asked_to_be_deleted_by(DeletionAskedBy::Holder);

    // when: the deletion is called off
    account.apply(
        &AccountEvent::DeletionCalledOff,
        &stamped(4, clock::at(4_000)),
    );

    // then: it is as before, signing in open to it again
    assert_eq!(account.pending_deletion(), None);
    assert!(!account.is_locked());
}

#[test]
fn an_account_asked_to_be_deleted_cannot_become_an_operator() {
    // given: an account asked to be deleted
    let account = asked_to_be_deleted_by(DeletionAskedBy::Holder);

    // expect: it cannot become an operator
    assert_eq!(
        account.decide(AccountCommand::BecomeOperator, &herself()),
        Err(AccountError::LockedCannotBecomeOperator)
    );
}

#[test]
fn nobody_asks_for_an_account_to_be_deleted_before_signing_up() {
    // expect: neither asking nor calling off can come first
    assert_eq!(
        Account::begin(
            AccountCommand::AskDeletion {
                until: clock::at(9_000),
                by: DeletionAskedBy::Holder,
            },
            &herself()
        ),
        Err(AccountError::NotSignedUpYet)
    );
    assert_eq!(
        Account::begin(
            AccountCommand::CallOffDeletion {
                by: DeletionAskedBy::Holder
            },
            &herself()
        ),
        Err(AccountError::NotSignedUpYet)
    );
}

#[test]
fn an_account_deleted_keeps_nothing_of_who_held_it_and_refuses_everything_after() {
    // given: an account with an operator's lock, TOTP and a key, asked to be deleted
    let mut account = asked_to_be_deleted_by(DeletionAskedBy::Holder);
    account.apply(&a_lock(), &stamped(4, clock::at(4_000)));
    account.apply(&AccountEvent::TotpAdded, &stamped(5, clock::at(4_100)));
    account.apply(
        &a_key_added("MacBook", KeyRole::Primary),
        &stamped(6, clock::at(4_200)),
    );

    // when: it is deleted
    let deleted = account
        .decide(AccountCommand::Delete, &Agent::System)
        .expect("an account asked to be deleted is deleted");
    assert_eq!(deleted, vec![AccountEvent::Deleted]);
    account.apply(&AccountEvent::Deleted, &stamped(7, clock::at(9_000)));

    // then: neither its name nor its address is left, nor any way in
    assert!(account.is_gone());
    assert_eq!(account.email().as_str(), "deleted@account.invalid");
    assert_eq!(account.name().as_str(), "Deleted");
    assert!(account.keys().is_empty());
    assert_eq!(account.totp(), None);
    assert_eq!(account.second_factor(), None);
    assert!(account.locks().is_empty());
    assert!(account.is_locked());
    assert!(account.is_locked_by_the_operator());
    // and: its snapshot holds nothing more
    let AccountEvent::Snapshotted { email, name, .. } = account.snapshot() else {
        panic!("an account's snapshot is a snapshot");
    };
    assert_eq!(email.as_str(), "deleted@account.invalid");
    assert_eq!(name.as_str(), "Deleted");
    assert_eq!(
        Account::from_first(&account.snapshot(), &stamped(8, clock::at(9_100))),
        Some(account.clone())
    );
    // and: everything after is refused, deleting again changing nothing
    assert_eq!(
        account.decide(AccountCommand::Verify, &herself()),
        Err(AccountError::Gone)
    );
    assert_eq!(
        account.decide(
            AccountCommand::CallOffDeletion {
                by: DeletionAskedBy::Operator
            },
            &herself()
        ),
        Err(AccountError::Gone)
    );
    assert_eq!(
        account.decide(AccountCommand::Delete, &Agent::System),
        Ok(vec![])
    );
}

#[test]
fn an_account_nobody_asked_to_delete_is_not_deleted() {
    // given: an account nobody asked to delete
    let account = a_verified_account();

    // expect: deleting it is refused
    assert_eq!(
        account.decide(AccountCommand::Delete, &Agent::System),
        Err(AccountError::NotAskedToBeDeleted)
    );
}

fn picture(id: &str) -> PictureId {
    PictureId::new(id).expect("a plain id")
}

fn decided(account: &mut Account, command: AccountCommand, version: u64) -> Vec<AccountEvent> {
    let events = account
        .decide(command, &herself())
        .expect("the command is taken");
    for event in &events {
        account.apply(event, &stamped(version, clock::at(5_000)));
    }

    events
}

#[test]
fn a_picture_is_changed_naming_the_one_shown_before() {
    // given: an account showing no picture
    let mut account = a_signed_up_account();

    // when: a first picture is shown
    let first = decided(
        &mut account,
        AccountCommand::ChangePicture {
            picture: picture("asset_1"),
            drop_previous: true,
        },
        2,
    );

    // then: it is shown, with nothing shown before
    assert_eq!(
        first,
        vec![AccountEvent::PictureChanged {
            picture: picture("asset_1"),
            previous: None,
            drop_previous: true,
        }]
    );
    assert_eq!(account.picture(), Some(&picture("asset_1")));
    // when: another picture replaces it, the first to be kept
    let second = decided(
        &mut account,
        AccountCommand::ChangePicture {
            picture: picture("asset_2"),
            drop_previous: false,
        },
        3,
    );
    // then: the change names the first as the one shown before
    assert_eq!(
        second,
        vec![AccountEvent::PictureChanged {
            picture: picture("asset_2"),
            previous: Some(picture("asset_1")),
            drop_previous: false,
        }]
    );
    assert_eq!(account.picture(), Some(&picture("asset_2")));
}

#[test]
fn showing_the_picture_already_shown_changes_nothing() {
    // given: an account showing a picture
    let mut account = a_signed_up_account();
    decided(
        &mut account,
        AccountCommand::ChangePicture {
            picture: picture("asset_1"),
            drop_previous: false,
        },
        2,
    );

    // expect: showing it again changes nothing
    assert_eq!(
        account.decide(
            AccountCommand::ChangePicture {
                picture: picture("asset_1"),
                drop_previous: true,
            },
            &herself()
        ),
        Ok(vec![])
    );
}

#[test]
fn a_picture_is_removed_once_and_removing_none_changes_nothing() {
    // given: an account showing a picture
    let mut account = a_signed_up_account();
    decided(
        &mut account,
        AccountCommand::ChangePicture {
            picture: picture("asset_1"),
            drop_previous: false,
        },
        2,
    );

    // when: it is removed, to be deleted
    let removed = decided(
        &mut account,
        AccountCommand::RemovePicture { drop: true },
        3,
    );

    // then: the removal names it and whether to delete it
    assert_eq!(
        removed,
        vec![AccountEvent::PictureRemoved {
            picture: picture("asset_1"),
            drop: true,
        }]
    );
    assert_eq!(account.picture(), None);
    // and: removing it again changes nothing
    assert_eq!(
        account.decide(AccountCommand::RemovePicture { drop: false }, &herself()),
        Ok(vec![])
    );
}

#[test]
fn a_picture_survives_a_snapshot_and_goes_with_the_account() {
    // given: an account showing a picture
    let mut account = a_signed_up_account();
    decided(
        &mut account,
        AccountCommand::ChangePicture {
            picture: picture("asset_1"),
            drop_previous: false,
        },
        2,
    );

    // when: the account is rebuilt from its snapshot
    let rebuilt = Account::from_first(&account.snapshot(), &stamped(3, clock::at(6_000)))
        .expect("a snapshot gives birth");

    // then: it still shows the picture
    assert_eq!(rebuilt.picture(), Some(&picture("asset_1")));
    // when: the account is deleted
    account.apply(&AccountEvent::Deleted, &stamped(4, clock::at(7_000)));
    // then: it shows no picture any more
    assert_eq!(account.picture(), None);
}
