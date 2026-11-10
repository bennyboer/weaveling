use std::sync::Arc;

use accounts_contract::stage;
use accounts_core::{
    AccountCatalog, AccountError, AccountId, AccountsError, DeletionAskedBy, EmailAddress,
    EmailAddresses, KEPT_BEFORE_DELETION, KIND, PendingDeletion, Stage,
};
use chores::Chore;
use eventsourcing::{AggregateId, Event, EventStore, ServiceError, Version};
use time::{Duration, OffsetDateTime};
use trash::Trash;

use crate::totp_bench;
use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";
const SIGNED_UP_AT: i64 = 1_700_000_000;
const A_MINUTE_LATER: i64 = SIGNED_UP_AT + 60;

async fn a_verified_account(wired: &Wired, email: &str) -> AccountId {
    wired
        .accounts
        .sign_up(email, "Someone", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(email, &wired.last_code_sent_to(email))
        .await
        .expect("the code sent should verify");

    wired
        .secrets
        .holder(&EmailAddress::new(email).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

async fn the_operator(wired: &Wired, email: &str) -> AccountId {
    let operator = a_verified_account(wired, email).await;
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    wired
        .accounts
        .become_operator(&operator, code.as_str())
        .await
        .expect("the code drawn makes an operator");

    operator
}

async fn another_operator(wired: &Wired, by: &AccountId, email: &str) -> AccountId {
    let operator = a_verified_account(wired, email).await;
    wired
        .accounts
        .appoint_operator(by, &operator, None)
        .await
        .expect("an operator appoints another");

    operator
}

async fn pending(wired: &Wired, id: &AccountId) -> Option<PendingDeletion> {
    wired
        .account(id)
        .await
        .expect("the account should exist")
        .state
        .pending_deletion()
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
async fn the_holder_asks_for_their_account_to_be_deleted_for_its_password_and_is_signed_out() {
    // given: an account signed in
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let (token, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in");

    // when: she asks for her account to be deleted with a wrong password
    let refused = wired.accounts.ask_deletion(&ada, "not her password").await;

    // then: it is refused, and nothing is asked
    assert!(matches!(refused, Err(AccountsError::PasswordWrong)));
    assert_eq!(pending(&wired, &ada).await, None);
    // when: she asks with her password, a minute later
    wired.clock.set(clock::at(A_MINUTE_LATER));
    let until = wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");
    // then: it is kept thirty days from now, asked by her
    assert_eq!(until, clock::at(A_MINUTE_LATER) + KEPT_BEFORE_DELETION);
    assert_eq!(
        pending(&wired, &ada).await,
        Some(PendingDeletion {
            until,
            asked_by: DeletionAskedBy::Holder,
        })
    );
    // and: her session is over
    assert_eq!(
        wired
            .accounts
            .signed_in(&token)
            .await
            .expect("asking should succeed"),
        None
    );
}

#[tokio::test]
async fn an_operator_deleting_their_account_stops_being_one_unless_the_last() {
    // given: the only operator
    let wired = wiring::wired();
    let grace = the_operator(&wired, GRACE).await;

    // when: she asks for her account to be deleted
    let refused = wired.accounts.ask_deletion(&grace, HER_PASSWORD).await;

    // then: it is refused, as the last operator, and she stays one
    assert!(matches!(refused, Err(AccountsError::LastOperator)));
    assert_eq!(pending(&wired, &grace).await, None);
    assert!(is_operator(&wired, &grace).await);
    // given: another operator
    let ada = another_operator(&wired, &grace, ADA).await;
    // when: she asks again
    wired
        .accounts
        .ask_deletion(&grace, HER_PASSWORD)
        .await
        .expect("no longer the last operator, she may");
    // then: she is no operator any more, her account to be deleted
    assert!(!is_operator(&wired, &grace).await);
    assert!(pending(&wired, &grace).await.is_some());
    // and: the other is the last operator now, and may not go too
    assert!(is_operator(&wired, &ada).await);
    assert!(matches!(
        wired.accounts.ask_deletion(&ada, HER_PASSWORD).await,
        Err(AccountsError::LastOperator)
    ));
}

#[tokio::test]
async fn the_operator_marks_an_account_for_deletion_and_only_the_operator_calls_it_off() {
    // given: an account and an operator
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let grace = the_operator(&wired, GRACE).await;

    // when: the operator marks the account for deletion
    let until = wired
        .accounts
        .mark_for_deletion(&grace, &ada, None)
        .await
        .expect("an operator marks an account");

    // then: it is to be deleted, as the operator asked
    assert_eq!(
        pending(&wired, &ada).await,
        Some(PendingDeletion {
            until,
            asked_by: DeletionAskedBy::Operator,
        })
    );
    // when: the holder signs in and tries to keep it
    let (pending_session, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in, only to see what became of her account");
    let refused = wired.accounts.keep_account(&pending_session).await;
    // then: it is refused, as only the operator calls it off
    assert!(matches!(
        refused,
        Err(AccountsError::Events(ServiceError::Refused(
            AccountError::DeletionAskedByTheOperator
        )))
    ));
    assert!(pending(&wired, &ada).await.is_some());
    // when: the operator keeps it
    wired
        .accounts
        .keep_account_of(&grace, &ada, None)
        .await
        .expect("the operator calls it off");
    // then: it is kept, and signs in again
    assert_eq!(pending(&wired, &ada).await, None);
    wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("a kept account signs in");
}

#[tokio::test]
async fn the_holder_keeps_the_account_they_asked_to_delete() {
    // given: an account its holder asked to delete
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");

    // when: she signs in
    let (pending_session, signed_in) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in, only to keep her account or sign out");

    // then: the session may do nothing but keep the account or sign out
    assert_eq!(signed_in.session.stage, Stage::DeletionPending);
    assert!(!signed_in.session.may_act());
    // when: she keeps it
    let (kept_session, kept) = wired
        .accounts
        .keep_account(&pending_session)
        .await
        .expect("she calls off what she asked");
    // then: it is no longer to be deleted, and she is signed in as before
    assert_eq!(pending(&wired, &ada).await, None);
    assert_eq!(kept.session.stage, Stage::SignedIn);
    assert!(
        wired
            .accounts
            .signed_in(&kept_session)
            .await
            .expect("asking should succeed")
            .is_some_and(|signed_in| signed_in.session.may_act())
    );
    // and: the session she kept it from is over
    assert_eq!(
        wired
            .accounts
            .signed_in(&pending_session)
            .await
            .expect("asking should succeed"),
        None
    );
}

#[tokio::test]
async fn only_a_session_opened_for_a_pending_deletion_keeps_an_account() {
    // given: an account signed in as ever
    let wired = wiring::wired();
    a_verified_account(&wired, ADA).await;
    let (signed_in, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in");

    // expect: keeping an account from it is refused, there being nothing to keep
    assert!(matches!(
        wired.accounts.keep_account(&signed_in).await,
        Err(AccountsError::NoDeletionPending)
    ));
}

#[tokio::test]
async fn only_an_operator_marks_another_account_or_keeps_it() {
    // given: two accounts, neither an operator, one asked to be deleted
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let grace = a_verified_account(&wired, GRACE).await;
    wired
        .accounts
        .ask_deletion(&grace, HER_PASSWORD)
        .await
        .expect("her password asks for it");

    // expect: the other may neither mark the first nor keep the second
    assert!(matches!(
        wired.accounts.mark_for_deletion(&ada, &grace, None).await,
        Err(AccountsError::NotAnOperator)
    ));
    assert!(matches!(
        wired.accounts.keep_account_of(&ada, &grace, None).await,
        Err(AccountsError::NotAnOperator)
    ));
    // and: nothing changed
    assert_eq!(pending(&wired, &ada).await, None);
    assert!(pending(&wired, &grace).await.is_some());
}

#[tokio::test]
async fn an_account_with_a_second_step_answers_it_before_it_is_offered_to_be_kept() {
    // given: ada with TOTP, her account asked to be deleted
    let set_up = totp_bench::with_a_second_factor_set_up_at(1_700_000_060).await;
    let served = &set_up.served;
    let now = served.at(1_700_000_120);
    served
        .wired
        .accounts
        .ask_deletion(&served.ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");

    // when: she signs in with her password
    let owing = served.signed_in().await;

    // then: her second step is asked first
    assert_eq!(served.stage_of(&owing).await, stage::SECOND_FACTOR_OWED);
    // when: she answers it
    let answered = served
        .answer(&owing, &totp_bench::code_of(&set_up.secret, now + 30))
        .await;
    // then: the session opens only to keep her account or sign out
    answered.assert_status_ok();
    let pending = totp_bench::cookie_set_by(&answered);
    assert_eq!(served.stage_of(&pending).await, stage::DELETION_PENDING);
}

#[tokio::test]
async fn a_session_signed_in_the_moment_deletion_is_asked_ends_with_the_others() {
    // given: an account signed in, the clock not moving on
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let (token, _) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in");

    // when: her account is asked to be deleted at that very moment
    wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");

    // then: the session signed in is over all the same
    assert_eq!(
        wired
            .accounts
            .signed_in(&token)
            .await
            .expect("asking should succeed"),
        None
    );
}

async fn due_at(wired: &Wired, moment: OffsetDateTime) -> Vec<String> {
    wired
        .trash
        .due(moment, 10)
        .await
        .expect("asking should succeed")
        .into_iter()
        .map(|trashed| trashed.id)
        .collect()
}

fn sweeping(wired: &Wired) -> &Arc<dyn Chore> {
    wired
        .chores
        .iter()
        .find(|chore| chore.named() == "sweep-deleted-accounts")
        .expect("accounts sweep those deleted")
}

#[tokio::test]
async fn an_account_asked_to_be_deleted_is_due_on_its_day_and_asking_again_moves_nothing() {
    // given: an account
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;

    // when: it is asked to be deleted, and asked again a day later
    let until = wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");
    wired.clock.set(clock::at(SIGNED_UP_AT) + Duration::days(1));
    wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("asking again changes nothing");

    // then: it is due on the first day asked, not a moment before
    assert!(
        due_at(&wired, until - Duration::seconds(1))
            .await
            .is_empty()
    );
    assert_eq!(due_at(&wired, until).await, vec![ada.to_string()]);
}

#[tokio::test]
async fn an_account_kept_by_its_holder_or_the_operator_is_due_no_more() {
    for by_the_operator in [false, true] {
        // given: an account asked to be deleted, and an operator
        let wired = wiring::wired();
        let ada = a_verified_account(&wired, ADA).await;
        let grace = the_operator(&wired, GRACE).await;
        let until = wired
            .accounts
            .ask_deletion(&ada, HER_PASSWORD)
            .await
            .expect("her password asks for it");

        // when: it is kept, by her signing in or by the operator
        if by_the_operator {
            wired
                .accounts
                .keep_account_of(&grace, &ada, None)
                .await
                .expect("the operator keeps it");
        } else {
            let (pending_session, _) = wired
                .accounts
                .begin_session(ADA, HER_PASSWORD)
                .await
                .expect("she signs in");
            wired
                .accounts
                .keep_account(&pending_session)
                .await
                .expect("she keeps it");
        }

        // then: it is due no more
        assert!(
            due_at(&wired, until).await.is_empty(),
            "kept by the operator: {by_the_operator}"
        );
    }
}

#[tokio::test]
async fn the_sweep_takes_an_account_whose_day_has_come_and_none_before() {
    // given: an account asked to be deleted, and the operator who marked another
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let grace = the_operator(&wired, GRACE).await;
    let until = wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");
    let hedy = a_verified_account(&wired, "hedy@example.com").await;
    wired
        .clock
        .set(clock::at(SIGNED_UP_AT) + Duration::days(10));
    let later = wired
        .accounts
        .mark_for_deletion(&grace, &hedy, None)
        .await
        .expect("the operator marks another account");

    // when: the sweep runs on ada's day
    wired.clock.set(until);
    let swept = sweeping(&wired)
        .carry_out()
        .await
        .expect("sweeping should succeed");

    // then: only ada is swept, and hedy waits for her own day
    assert_eq!(swept, 1);
    assert_eq!(due_at(&wired, later).await, vec![hedy.to_string()]);
}

#[tokio::test]
async fn an_account_swept_keeps_nothing_of_its_holder_and_is_listed_no_more() {
    // given: an account asked to be deleted, listed among the accounts
    let wired = wiring::wired();
    let ada = a_verified_account(&wired, ADA).await;
    let until = wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("her password asks for it");
    wired.settle().await;
    assert!(listed(&wired).await.contains(&ada));

    // when: the sweep runs on its day
    wired.clock.set(until);
    sweeping(&wired)
        .carry_out()
        .await
        .expect("sweeping should succeed");
    wired.settle().await;

    // then: the account is gone
    let gone = wired
        .account(&ada)
        .await
        .expect("the account should exist")
        .state;
    assert!(gone.is_gone());
    // and: its history holds only the snapshot of it gone, nothing naming or reaching her
    let history = wired
        .store
        .read_from(&AggregateId::from(&ada), KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(history.len(), 1);
    assert!(history[0].event.is_snapshot());
    let written = format!("{:?}", history[0].event);
    assert!(
        !written.contains(ADA) && !written.contains("Someone"),
        "{written}"
    );
    // and: it is listed no more, nor due in the trash
    assert!(!listed(&wired).await.contains(&ada));
    assert!(due_at(&wired, until).await.is_empty());
}

async fn listed(wired: &Wired) -> Vec<AccountId> {
    wired
        .catalog
        .listed_after(None, 100)
        .await
        .expect("listing should succeed")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
}
