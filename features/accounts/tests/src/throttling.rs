use accounts_core::{AccountServiceError, EmailAddress, Guarded, Throttles};
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

async fn wrong_times(wired: &Wired, email: &str, times: usize) {
    for _ in 0..times {
        assert!(matches!(
            wired.accounts.sign_in(email, NOT_HERS).await,
            Err(AccountServiceError::SignInRefused)
        ));
    }
}

fn waits<T>(tried: Result<T, AccountServiceError>) -> Option<Duration> {
    match tried {
        Err(AccountServiceError::Throttled { wait }) => Some(wait),
        _ => None,
    }
}

fn after(minutes: i64) -> time::OffsetDateTime {
    wiring::at(1_700_000_000) + Duration::minutes(minutes)
}

#[tokio::test]
async fn five_wrong_passwords_are_free_and_the_next_try_must_wait_a_minute() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;

    wrong_times(&wired, ADA, 5).await;

    assert_eq!(
        waits(wired.accounts.sign_in(ADA, HER_PASSWORD).await),
        Some(Duration::minutes(1)),
        "past the free tries not even the right password is checked until the wait is over"
    );
}

#[tokio::test]
async fn once_the_wait_is_over_the_right_password_signs_in_and_the_count_starts_over() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    wrong_times(&wired, ADA, 5).await;

    wired.clock.set(after(1));
    wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("the wait is over and the password is right");

    assert_eq!(
        wired
            .secrets
            .tries(
                Guarded::SigningIn,
                &EmailAddress::new(ADA).expect("a plain address")
            )
            .await
            .expect("reading should succeed"),
        None
    );
}

#[tokio::test]
async fn each_wrong_password_past_the_free_ones_doubles_the_wait() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    wrong_times(&wired, ADA, 5).await;

    wired.clock.set(after(1));
    wrong_times(&wired, ADA, 1).await;

    assert_eq!(
        waits(wired.accounts.sign_in(ADA, HER_PASSWORD).await),
        Some(Duration::minutes(2))
    );
}

#[tokio::test]
async fn a_try_refused_for_waiting_is_not_counted() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    wrong_times(&wired, ADA, 5).await;
    for _ in 0..3 {
        assert!(waits(wired.accounts.sign_in(ADA, NOT_HERS).await).is_some());
    }

    wired.clock.set(after(1));
    wrong_times(&wired, ADA, 1).await;

    assert_eq!(
        waits(wired.accounts.sign_in(ADA, HER_PASSWORD).await),
        Some(Duration::minutes(2)),
        "only tries that checked a password count, or impatience alone would lengthen the wait"
    );
}

#[tokio::test]
async fn an_address_nobody_has_waits_alike() {
    let wired = wiring::wired();

    wrong_times(&wired, "nobody@example.com", 5).await;

    assert_eq!(
        waits(wired.accounts.sign_in("nobody@example.com", NOT_HERS).await),
        Some(Duration::minutes(1)),
        "waiting only where an account stands would tell whose address it is"
    );
}

#[tokio::test]
async fn the_right_password_clears_the_count_even_before_the_address_is_confirmed() {
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    wrong_times(&wired, ADA, 4).await;

    assert!(matches!(
        wired.accounts.sign_in(ADA, HER_PASSWORD).await,
        Err(AccountServiceError::NotVerifiedYet)
    ));
    wrong_times(&wired, ADA, 4).await;

    assert!(matches!(
        wired.accounts.sign_in(ADA, NOT_HERS).await,
        Err(AccountServiceError::SignInRefused)
    ));
}

#[tokio::test]
async fn tries_a_day_old_are_forgotten() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    wrong_times(&wired, ADA, 5).await;

    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(1));
    wrong_times(&wired, ADA, 1).await;

    assert_eq!(
        wired
            .secrets
            .tries(
                Guarded::SigningIn,
                &EmailAddress::new(ADA).expect("a plain address")
            )
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
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    for _ in 0..2 {
        wired
            .accounts
            .send_code_again(ADA)
            .await
            .expect("the free sends go out");
    }

    assert_eq!(
        waits(wired.accounts.send_code_again(ADA).await),
        Some(Duration::minutes(1)),
        "signing up mails a code too, so it counts as one"
    );
    assert_eq!(mails_to(&wired, ADA), 3);
}

#[tokio::test]
async fn asking_to_reset_counts_with_the_codes_sent_again() {
    let wired = wiring::wired();
    ada_signed_up(&wired, true).await;
    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("a free send");
    wired
        .accounts
        .ask_to_reset_password(ADA)
        .await
        .expect("a free send");

    assert!(waits(wired.accounts.ask_to_reset_password(ADA).await).is_some());
}

#[tokio::test]
async fn signing_up_again_and_again_with_one_address_must_wait_too() {
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    for _ in 0..2 {
        wired
            .accounts
            .sign_up(ADA, "Ada", HER_PASSWORD)
            .await
            .expect("a taken address is answered like any other");
    }

    assert!(
        waits(wired.accounts.sign_up(ADA, "Ada", HER_PASSWORD).await).is_some(),
        "or anyone could fill a stranger's inbox by signing up with it"
    );
    assert_eq!(mails_to(&wired, ADA), 3);
}

#[tokio::test]
async fn an_address_nobody_has_waits_to_be_sent_to_alike() {
    let wired = wiring::wired();
    for _ in 0..3 {
        wired
            .accounts
            .ask_to_reset_password("nobody@example.com")
            .await
            .expect("asking is accepted whoever asks");
    }

    assert_eq!(
        waits(wired.accounts.send_code_again("nobody@example.com").await),
        Some(Duration::minutes(1)),
        "waiting only where an account stands would tell whose address it is"
    );
}

#[tokio::test]
async fn a_send_refused_for_waiting_is_not_counted() {
    let wired = wiring::wired();
    ada_signed_up(&wired, false).await;
    for _ in 0..2 {
        wired
            .accounts
            .send_code_again(ADA)
            .await
            .expect("the free sends go out");
    }
    for _ in 0..3 {
        assert!(waits(wired.accounts.send_code_again(ADA).await).is_some());
    }

    wired.clock.set(after(1));

    wired
        .accounts
        .send_code_again(ADA)
        .await
        .expect("the wait is over");
    assert_eq!(mails_to(&wired, ADA), 4);
}

#[tokio::test]
async fn a_sign_up_refused_for_what_it_asks_is_not_counted() {
    let wired = wiring::wired();
    for _ in 0..3 {
        assert!(matches!(
            wired.accounts.sign_up(ADA, "Ada", "short").await,
            Err(AccountServiceError::InvalidPassword(_))
        ));
    }

    ada_signed_up(&wired, false).await;
}
