use accounts_core::{AccountId, AccountServiceError, EmailAddress, Guarded, Throttles};
use time::Duration;

use crate::wiring::{ADA, HER_PASSWORD, Wired, at, wired};

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

fn waits(signed_in: Result<AccountId, AccountServiceError>) -> Option<Duration> {
    match signed_in {
        Err(AccountServiceError::Throttled { wait }) => Some(wait),
        _ => None,
    }
}

fn after(minutes: i64) -> time::OffsetDateTime {
    at(1_700_000_000) + Duration::minutes(minutes)
}

#[tokio::test]
async fn five_wrong_passwords_are_free_and_the_next_try_must_wait_a_minute() {
    let wired = wired();
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
    let wired = wired();
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
    let wired = wired();
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
    let wired = wired();
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
    let wired = wired();

    wrong_times(&wired, "nobody@example.com", 5).await;

    assert_eq!(
        waits(wired.accounts.sign_in("nobody@example.com", NOT_HERS).await),
        Some(Duration::minutes(1)),
        "waiting only where an account stands would tell whose address it is"
    );
}

#[tokio::test]
async fn the_right_password_clears_the_count_even_before_the_address_is_confirmed() {
    let wired = wired();
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
    let wired = wired();
    ada_signed_up(&wired, true).await;
    wrong_times(&wired, ADA, 5).await;

    wired.clock.set(at(1_700_000_000) + Duration::days(1));
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
