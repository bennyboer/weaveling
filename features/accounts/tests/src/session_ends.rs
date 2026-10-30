use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use accounts_core::{
    AccountsCore, SecretStores, SecretsError, Session, SessionToken, Sessions, TokenDigest,
};
use accounts_secrets::InMemorySecrets;
use acting::SessionEnd;
use async_trait::async_trait;
use clock::Clock;
use tokio::task::JoinHandle;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const SETTLING: Duration = Duration::from_millis(50);
const SOON: Duration = Duration::from_secs(1);
const QUIET: Duration = Duration::from_millis(150);

async fn ada_signed_in(wired: &Wired) -> (SessionToken, SessionEnd) {
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");

    a_session(wired).await
}

async fn a_session(wired: &Wired) -> (SessionToken, SessionEnd) {
    let (token, signed_in) = wired
        .accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in");
    let end = wired.accounts.session_end(&token, signed_in.session.until);

    (token, end)
}

pub(crate) async fn awaited(end: SessionEnd) -> JoinHandle<()> {
    let waiting = tokio::spawn(async move { end.reached().await });
    tokio::time::sleep(SETTLING).await;

    waiting
}

pub(crate) async fn reached(waiting: JoinHandle<()>) -> bool {
    tokio::time::timeout(SOON, waiting).await.is_ok()
}

pub(crate) async fn still_running(waiting: &mut JoinHandle<()>) -> bool {
    tokio::time::timeout(QUIET, waiting).await.is_err()
}

#[tokio::test]
async fn signing_out_reaches_the_session_s_end_at_once() {
    let wired = wiring::wired();
    let (token, end) = ada_signed_in(&wired).await;
    let waiting = awaited(end).await;

    wired
        .accounts
        .end_session(&token)
        .await
        .expect("signing out should succeed");

    assert!(reached(waiting).await);
}

#[tokio::test]
async fn signing_out_elsewhere_leaves_this_session_running() {
    let wired = wiring::wired();
    let (_, end) = ada_signed_in(&wired).await;
    let (elsewhere, _) = a_session(&wired).await;
    let mut waiting = awaited(end).await;

    wired
        .accounts
        .end_session(&elsewhere)
        .await
        .expect("signing out should succeed");

    assert!(
        still_running(&mut waiting).await,
        "asked again, this session still counts"
    );
}

#[tokio::test]
async fn a_session_s_end_is_reached_at_its_last_moment() {
    let wired = wiring::wired();
    let (token, _) = ada_signed_in(&wired).await;
    let until = wired.clock.now() + time::Duration::milliseconds(300);
    let mut waiting = awaited(wired.accounts.session_end(&token, until)).await;

    assert!(
        still_running(&mut waiting).await,
        "not before its last moment"
    );
    assert!(reached(waiting).await);
}

#[tokio::test]
async fn a_session_already_over_is_ended_from_the_start() {
    let wired = wiring::wired();
    let (token, end) = ada_signed_in(&wired).await;
    wired
        .accounts
        .end_session(&token)
        .await
        .expect("signing out should succeed");

    assert!(reached(tokio::spawn(async move { end.reached().await })).await);
}

struct Unreachable {
    kept: Arc<InMemorySecrets>,
    down: AtomicBool,
}

#[async_trait]
impl Sessions for Unreachable {
    async fn open(&self, digest: &TokenDigest, session: &Session) -> Result<(), SecretsError> {
        self.kept.open(digest, session).await
    }

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError> {
        if self.down.load(Ordering::SeqCst) {
            return Err(SecretsError::Backend(
                "the sessions cannot be reached".into(),
            ));
        }

        self.kept.find(digest).await
    }

    async fn end(&self, digest: &TokenDigest) -> Result<(), SecretsError> {
        self.kept.end(digest).await
    }
}

#[tokio::test]
async fn a_session_that_cannot_be_asked_about_is_not_taken_for_ended() {
    let wired = wiring::wired();
    ada_signed_in(&wired).await;
    let sessions = Arc::new(Unreachable {
        kept: wired.secrets.clone(),
        down: AtomicBool::new(false),
    });
    let accounts = AccountsCore::new(
        wired.store.clone(),
        SecretStores {
            sessions: sessions.clone(),
            ..SecretStores::kept_by(wired.secrets.clone(), wired.throttles.clone())
        },
        wired.catalog.clone(),
        wired.mailbox.clone(),
        wired.clock.clone(),
    );
    let (token, signed_in) = accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in");
    let (elsewhere, _) = accounts
        .begin_session(ADA, HER_PASSWORD)
        .await
        .expect("she signs in elsewhere");
    let mut waiting = awaited(accounts.session_end(&token, signed_in.session.until)).await;

    sessions.down.store(true, Ordering::SeqCst);
    accounts
        .end_session(&elsewhere)
        .await
        .expect("signing out should succeed");

    assert!(
        still_running(&mut waiting).await,
        "a store out of reach for a moment signs nobody out"
    );
}
