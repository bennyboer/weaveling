use accounts_core::{AccountsError, Guarded};
use futures_util::future;
use throttling::Throttles;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const NOT_HERS: &str = "not her password at all";

async fn ada_signed_up(wired: &Wired, confirmed: bool) {
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
}

async fn signing_in_wrongly(wired: &Wired, email: &str, times: usize) {
    for _ in 0..times {
        assert!(matches!(
            wired.accounts.sign_in(email, NOT_HERS).await,
            Err(AccountsError::SignInRefused)
        ));
    }
}

fn waits<T>(tried: Result<T, AccountsError>) -> Option<Duration> {
    match tried {
        Err(AccountsError::Throttled { wait }) => Some(wait),
        _ => None,
    }
}

fn after(minutes: i64) -> time::OffsetDateTime {
    clock::at(1_700_000_000) + Duration::minutes(minutes)
}

#[tokio::test]
async fn five_wrong_passwords_are_free_and_the_next_try_must_wait_a_minute() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;

    // when: five wrong passwords are tried
    signing_in_wrongly(&wired, ADA, 5).await;

    // then: the next try must wait a minute
    assert_eq!(
        waits(wired.accounts.sign_in(ADA, HER_PASSWORD).await),
        Some(Duration::minutes(1)),
        "past the free tries not even the right password is checked until the wait is over"
    );
}

#[tokio::test]
async fn once_the_wait_is_over_the_right_password_signs_in_and_the_count_starts_over() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    // and: five wrong passwords tried
    signing_in_wrongly(&wired, ADA, 5).await;

    // when: the right one is tried once the wait is over
    wired.clock.set(after(1));
    wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("the wait is over and the password is right");

    // then: no tries are counted
    assert_eq!(
        wired
            .throttles
            .tries(Guarded::SigningIn.as_str(), ADA)
            .await
            .expect("reading should succeed"),
        None
    );
}

#[tokio::test]
async fn each_wrong_password_past_the_free_ones_doubles_the_wait() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    // and: five wrong passwords tried
    signing_in_wrongly(&wired, ADA, 5).await;

    // when: another wrong one is tried once the wait is over
    wired.clock.set(after(1));
    signing_in_wrongly(&wired, ADA, 1).await;

    // then: the next try must wait twice as long
    assert_eq!(
        waits(wired.accounts.sign_in(ADA, HER_PASSWORD).await),
        Some(Duration::minutes(2))
    );
}

#[tokio::test]
async fn a_try_refused_for_waiting_is_not_counted() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    // and: five wrong passwords tried
    signing_in_wrongly(&wired, ADA, 5).await;
    // and: three more tries refused for waiting
    for _ in 0..3 {
        assert!(waits(wired.accounts.sign_in(ADA, NOT_HERS).await).is_some());
    }

    // when: another wrong one is tried once the wait is over
    wired.clock.set(after(1));
    signing_in_wrongly(&wired, ADA, 1).await;

    // then: the wait has doubled only once
    assert_eq!(
        waits(wired.accounts.sign_in(ADA, HER_PASSWORD).await),
        Some(Duration::minutes(2)),
        "only tries that checked a password count, or impatience alone would lengthen the wait"
    );
}

#[tokio::test]
async fn an_address_nobody_has_waits_alike() {
    // given: no accounts
    let wired = wiring::wired();

    // when: five wrong passwords are tried for an address nobody has
    signing_in_wrongly(&wired, "nobody@example.com", 5).await;

    // then: the next try must wait a minute
    assert_eq!(
        waits(wired.accounts.sign_in("nobody@example.com", NOT_HERS).await),
        Some(Duration::minutes(1)),
        "waiting only where an account stands would tell whose address it is"
    );
}

#[tokio::test]
async fn the_right_password_clears_the_count_even_before_the_address_is_confirmed() {
    // given: an unverified account
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    // and: four wrong passwords tried
    signing_in_wrongly(&wired, ADA, 4).await;

    // when: the right one is tried
    assert!(matches!(
        wired.accounts.sign_in(ADA, HER_PASSWORD).await,
        Err(AccountsError::NotVerifiedYet)
    ));
    // and: four wrong ones again
    signing_in_wrongly(&wired, ADA, 4).await;

    // then: the next wrong one is still checked
    assert!(matches!(
        wired.accounts.sign_in(ADA, NOT_HERS).await,
        Err(AccountsError::SignInRefused)
    ));
}

#[tokio::test]
async fn tries_a_day_old_are_forgotten() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    // and: five wrong passwords tried
    signing_in_wrongly(&wired, ADA, 5).await;

    // when: another wrong one is tried a day later
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(1));
    signing_in_wrongly(&wired, ADA, 1).await;

    // then: only that one is counted
    assert_eq!(
        wired
            .throttles
            .tries(Guarded::SigningIn.as_str(), ADA)
            .await
            .expect("reading should succeed")
            .map(|tries| tries.counted),
        Some(1),
        "a password mistyped yesterday is no reason to wait today"
    );
}

fn mails_to(wired: &Wired, email: &str) -> usize {
    wired.mailbox.sent_to(email).len()
}

#[tokio::test]
async fn signing_up_and_two_codes_again_are_free_and_the_next_must_wait_a_minute() {
    // given: an unverified account
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    // and: its code sent again twice
    for _ in 0..2 {
        wired
            .accounts
            .send_code_again(ADA)
            .await
            .expect("the free sends go out");
    }

    // then: sending it again must wait a minute
    assert_eq!(
        waits(wired.accounts.send_code_again(ADA).await),
        Some(Duration::minutes(1)),
        "signing up mails a code too, so it counts as one"
    );
    // and: three mails went out
    assert_eq!(mails_to(&wired, ADA), 3);
}

#[tokio::test]
async fn asking_to_reset_counts_with_the_codes_sent_again() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    // and: its code sent again
    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("a free send");
    // and: a reset asked for
    wired
        .accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("a free send");

    // then: asking for a reset again must wait
    assert!(waits(wired.accounts.ask_to_reset_password(ADA).await).is_some());
}

#[tokio::test]
async fn signing_up_again_and_again_with_one_address_must_wait_too() {
    // given: an unverified account
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    // and: its address signed up with twice more
    for _ in 0..2 {
        wired
            .accounts
            .sign_up(ADA, "Ada", HER_PASSWORD)
            .await
            .expect("a taken address is answered like any other");
    }

    // then: signing up with it again must wait
    assert!(
        waits(wired.accounts.sign_up(ADA, "Ada", HER_PASSWORD).await).is_some(),
        "or anyone could fill a stranger's inbox by signing up with it"
    );
    // and: three mails went out
    assert_eq!(mails_to(&wired, ADA), 3);
}

#[tokio::test]
async fn an_address_nobody_has_waits_to_be_sent_to_alike() {
    // given: three resets asked for an address nobody has
    let wired = wiring::wired();
    for _ in 0..3 {
        wired
            .accounts
            .ask_to_reset_password("nobody@example.com")
            .await
            .expect("asking is accepted whoever asks");
    }

    // then: sending a code to it must wait a minute
    assert_eq!(
        waits(wired.accounts.send_code_again("nobody@example.com").await),
        Some(Duration::minutes(1)),
        "waiting only where an account stands would tell whose address it is"
    );
}

#[tokio::test]
async fn a_send_refused_for_waiting_is_not_counted() {
    // given: an unverified account
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    // and: its code sent again twice
    for _ in 0..2 {
        wired
            .accounts
            .send_code_again(ADA)
            .await
            .expect("the free sends go out");
    }
    // and: three more sends refused for waiting
    for _ in 0..3 {
        assert!(waits(wired.accounts.send_code_again(ADA).await).is_some());
    }

    // when: a minute passes
    wired.clock.set(after(1));

    // then: the code is sent again
    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("the wait is over");
    // and: four mails went out
    assert_eq!(mails_to(&wired, ADA), 4);
}

#[tokio::test]
async fn a_sign_up_refused_for_what_it_asks_is_not_counted() {
    // given: three sign-ups refused for a short password
    let wired = wiring::wired();
    for _ in 0..3 {
        assert!(matches!(
            wired.accounts.sign_up(ADA, "Ada", "short").await,
            Err(AccountsError::InvalidPassword(_))
        ));
    }

    // then: signing up as asked is not kept waiting
    ada_signed_up(&wired, false).await;
}

#[tokio::test]
async fn wrong_passwords_sent_at_once_are_checked_no_more_often_than_the_free_tries() {
    // given: a verified account
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;

    // when: twenty wrong passwords are sent at once
    let told = future::join_all((0..20).map(|_| wired.accounts.sign_in(ADA, NOT_HERS))).await;

    // then: five are checked and the rest must wait
    let checked = told
        .iter()
        .filter(|told| matches!(told, Err(AccountsError::SignInRefused)))
        .count();
    let waiting = told.iter().filter(|told| waits_for(told)).count();
    assert_eq!(
        (checked, waiting),
        (5, 15),
        "guesses sent together must not all read a count under the limit before any is counted"
    );
}

fn waits_for<T>(told: &Result<T, AccountsError>) -> bool {
    matches!(told, Err(AccountsError::Throttled { .. }))
}

#[tokio::test]
async fn mail_anyone_may_have_sent_is_capped_across_the_instance_each_hour() {
    // given: an instance sending at most two such mails an hour, at twenty to the hour and a little
    let wired = wiring::wired();
    let accounts = wired.accounts.clone().capping_mail(2);
    let asked = |address: &'static str| {
        let accounts = accounts.clone();
        async move { accounts.ask_to_reset_password(address).await }
    };

    // when: three strangers' addresses are asked to be reset, each once
    asked("one@example.com").await.expect("the first is sent");
    asked("two@example.com").await.expect("the second is sent");
    let third = asked("three@example.com").await;

    // then: the third waits for the next hour, though no address was asked twice
    assert_eq!(
        waits(third),
        Some(Duration::minutes(46) + Duration::seconds(40)),
        "a cap by address alone lets anyone mail any number of strangers"
    );
    // and: in the next hour it is sent
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::minutes(47));
    asked("three@example.com")
        .await
        .expect("each hour is counted afresh");
}
