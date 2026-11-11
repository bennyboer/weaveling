use std::sync::Arc;

use accounts_core::{AccountId, Session, Sessions, Stage, TokenDigest};
use chores::Chore;
use clock::Clock;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

fn forgetting(wired: &Wired) -> &Arc<dyn Chore> {
    wired
        .chores
        .iter()
        .find(|chore| chore.named() == "forget-run-out-sessions-and-codes")
        .expect("accounts forget what ran out")
}

async fn a_session_until(wired: &Wired, named: &str, until: time::OffsetDateTime) -> TokenDigest {
    let digest = TokenDigest::kept(format!("session {named}"));
    wired
        .open(
            &digest,
            &Session {
                account: AccountId::generate(clock::at(1_000)),
                stage: Stage::SignedIn,
                begun: until - Session::LASTS,
                until,
            },
        )
        .await
        .expect("opening should succeed");

    digest
}

async fn held(wired: &Wired, digest: &TokenDigest) -> bool {
    Sessions::find(&*wired.secrets, digest)
        .await
        .expect("reading should succeed")
        .is_some()
}

#[tokio::test]
async fn sessions_and_codes_run_out_a_day_ago_are_forgotten_every_hour() {
    // given: a code sent on signing up, run out a day and a second ago
    let wired = wiring::wired();
    let now = wired.clock.now();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let later = now + Duration::minutes(15) + Duration::days(1) + Duration::SECOND;
    // and: a session run out as long ago, one run out a second short of a day ago, and one still open
    let long_gone = a_session_until(&wired, "long gone", now).await;
    let just_gone = a_session_until(
        &wired,
        "just gone",
        later - Duration::days(1) + Duration::SECOND,
    )
    .await;
    let open = a_session_until(&wired, "open", later + Duration::days(1)).await;
    wired.clock.set(later);

    // when: the chore is carried out
    let chore = forgetting(&wired);
    let forgotten = chore.carry_out().await.expect("forgetting should succeed");

    // then: the code and the session run out a day ago are forgotten, and only they
    assert_eq!(forgotten, 2);
    assert!(!held(&wired, &long_gone).await);
    assert!(
        held(&wired, &just_gone).await,
        "one run out a little while ago is still told it ran out"
    );
    assert!(held(&wired, &open).await);
    // and: it comes round every hour
    assert_eq!(chore.every().as_secs(), 3_600);
}
