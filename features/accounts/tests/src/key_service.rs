use accounts_core::{
    AccountError, AccountId, AccountsCore, AccountsError, EmailAddress, EmailAddresses,
    KeyCeremonies, KeyCredentials, KeyId, KeyRole, SecondFactors, SecondStepWay, SessionToken,
    Stage,
};
use eventsourcing::ServiceError;
use serde_json::{Value, json};
use time::Duration;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse, Url};

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const SERVED_AT: &str = "https://weaveling.example.com";
const GRACE: &str = "grace@example.com";

struct Bench {
    wired: Wired,
    accounts: AccountsCore,
}

async fn signed_up(wired: &Wired, email: &str) -> AccountId {
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

fn bench() -> Bench {
    let wired = wiring::wired();
    let accounts = wired
        .accounts
        .clone()
        .holding_key_ceremonies(KeyCeremonies::serving_at(SERVED_AT).expect("a public URL"));

    Bench { wired, accounts }
}

fn made_by(authenticator: &mut SoftPasskey, asked: Value) -> Value {
    let asked: CreationChallengeResponse =
        serde_json::from_value(asked).expect("the browser is asked in the WebAuthn shape");
    let made = authenticator
        .do_registration(Url::parse(SERVED_AT).expect("a URL"), asked)
        .expect("the authenticator makes a key");

    serde_json::to_value(made).expect("the answer is plain data")
}

/// The software passkey keeps no credential on itself, so a passkey's options are eased for it; the server asks for a resident one and never checks it was.
fn not_resident(mut asked: Value) -> Value {
    asked["publicKey"]["authenticatorSelection"]["residentKey"] = "discouraged".into();
    asked["publicKey"]["authenticatorSelection"]["requireResidentKey"] = false.into();

    asked
}

fn touch_of(authenticator: &mut SoftPasskey, asked: Value) -> Value {
    let asked: RequestChallengeResponse =
        serde_json::from_value(asked).expect("the browser is asked in the WebAuthn shape");
    let touched = authenticator
        .do_authentication(Url::parse(SERVED_AT).expect("a URL"), asked)
        .expect("the authenticator signs the challenge");

    serde_json::to_value(touched).expect("the answer is plain data")
}

impl Bench {
    async fn session_of(&self, email: &str) -> SessionToken {
        self.accounts
            .begin_session(email, HER_PASSWORD)
            .await
            .expect("signing in should succeed")
            .0
    }

    async fn answered(&self, account: &AccountId) -> Value {
        self.answered_by(&mut SoftPasskey::new(true), account).await
    }

    async fn answered_by(&self, authenticator: &mut SoftPasskey, account: &AccountId) -> Value {
        let asked = self
            .accounts
            .begin_adding_key(account, KeyRole::SecondFactor)
            .await
            .expect("adding begins");

        made_by(authenticator, asked)
    }

    async fn touched_by(&self, authenticator: &mut SoftPasskey, owing: &SessionToken) -> Value {
        let asked = self
            .accounts
            .begin_answering_with_key(owing)
            .await
            .expect("answering begins");

        touch_of(authenticator, asked)
    }

    async fn added(
        &self,
        account: &AccountId,
        name: &str,
        session: &SessionToken,
    ) -> accounts_core::AddedKey {
        let answer = self.answered(account).await;

        self.accounts
            .finish_adding_key(account, name, &answer, session)
            .await
            .expect("the key is added")
    }

    async fn is_open(&self, session: &SessionToken) -> bool {
        self.accounts
            .signed_in(session)
            .await
            .expect("reading should succeed")
            .is_some()
    }
}

fn refused_for(error: AccountsError) -> Option<AccountError> {
    match error {
        AccountsError::Events(ServiceError::Refused(refused)) => Some(refused),
        _ => None,
    }
}

#[tokio::test]
async fn a_key_is_added_for_the_second_step_under_its_name_with_ten_recovery_codes() {
    // given: a signed-in account without a second factor
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let elsewhere = bench.session_of(ADA).await;
    assert!(bench.is_open(&elsewhere).await);
    bench
        .wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::seconds(1));

    // when: a key the authenticator made is added
    let added = bench.added(&ada, "YubiKey blue", &session).await;

    // then: the account holds it, named, answering the second step
    let account = bench
        .wired
        .account(&ada)
        .await
        .expect("the account should exist")
        .state;
    let held = &account.keys()[&added.key];
    assert_eq!(held.name.as_str(), "YubiKey blue");
    assert_eq!(held.role, KeyRole::SecondFactor);
    assert!(account.second_factor().is_some());
    // and: its credential is kept
    assert_eq!(
        bench
            .wired
            .secrets
            .credentials_of(&ada)
            .await
            .expect("reading should succeed")
            .iter()
            .map(|(key, _)| *key)
            .collect::<Vec<_>>(),
        vec![added.key]
    );
    // and: ten recovery codes come with it, as the first second factor
    assert_eq!(added.recovery_codes.map(|codes| codes.len()), Some(10));
    // and: the session goes on as a new one, every other one ended
    assert!(bench.is_open(&added.session).await);
    assert!(!bench.is_open(&session).await);
    assert!(!bench.is_open(&elsewhere).await);
}

#[tokio::test]
async fn a_key_beside_a_second_factor_brings_no_new_codes() {
    // given: an account with a key answering its second step
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let first = bench.added(&ada, "YubiKey blue", &session).await;
    let codes_before = bench
        .wired
        .secrets
        .recovery_codes_of(&ada)
        .await
        .expect("reading should succeed");

    // when: another key is added
    let second = bench.added(&ada, "YubiKey green", &first.session).await;

    // then: no codes come with it, and the ones kept are the first key's still
    assert!(second.recovery_codes.is_none());
    assert_eq!(
        bench
            .wired
            .secrets
            .recovery_codes_of(&ada)
            .await
            .expect("reading should succeed"),
        codes_before
    );
    // and: the account holds both
    assert_eq!(
        bench
            .wired
            .account(&ada)
            .await
            .expect("the account should exist")
            .state
            .keys()
            .len(),
        2
    );
}

#[tokio::test]
async fn an_answer_with_nothing_under_way_adds_no_key() {
    // given: a signed-in account
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let answer = bench.answered(&ada).await;
    bench
        .accounts
        .finish_adding_key(&ada, "YubiKey blue", &answer, &session)
        .await
        .expect("the key is added");
    let session = bench.session_of(ADA).await;

    // when: the same answer is given once more
    let again = bench
        .accounts
        .finish_adding_key(&ada, "YubiKey green", &answer, &session)
        .await;

    // then: it is refused, nothing being under way
    assert!(
        matches!(again, Err(AccountsError::NoKeyBeingAdded)),
        "{again:?}"
    );
    // when: an adding is begun and answered five minutes later
    let late = bench.answered(&ada).await;
    bench
        .wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::minutes(5));
    let after = bench
        .accounts
        .finish_adding_key(&ada, "YubiKey green", &late, &session)
        .await;
    // then: it is refused as well
    assert!(
        matches!(after, Err(AccountsError::NoKeyBeingAdded)),
        "{after:?}"
    );
}

#[tokio::test]
async fn an_answer_to_another_accounts_adding_makes_no_key() {
    // given: two accounts adding a key at once
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let grace = signed_up(&bench.wired, GRACE).await;
    let session = bench.session_of(ADA).await;
    let _hers = bench
        .accounts
        .begin_adding_key(&ada, KeyRole::SecondFactor)
        .await
        .expect("adding begins");
    let theirs = bench.answered(&grace).await;

    // when: the second account's answer is given for the first
    let crossed = bench
        .accounts
        .finish_adding_key(&ada, "YubiKey blue", &theirs, &session)
        .await;

    // then: no key is made
    assert!(
        matches!(crossed, Err(AccountsError::KeyRefused)),
        "{crossed:?}"
    );
    // and: nor is one by an answer that is none
    let _again = bench.answered(&ada).await;
    let nonsense = bench
        .accounts
        .finish_adding_key(&ada, "YubiKey blue", &json!({ "id": "nonsense" }), &session)
        .await;
    assert!(
        matches!(nonsense, Err(AccountsError::KeyRefused)),
        "{nonsense:?}"
    );
}

#[tokio::test]
async fn a_name_another_key_has_is_refused_without_spending_the_adding() {
    // given: an account holding a key, adding another
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let first = bench.added(&ada, "YubiKey blue", &session).await;
    let answer = bench.answered(&ada).await;

    // when: the new key is given the first one's name
    let taken = bench
        .accounts
        .finish_adding_key(&ada, "YubiKey blue", &answer, &first.session)
        .await
        .map(|_| ())
        .map_err(refused_for);

    // then: it is refused
    assert_eq!(taken, Err(Some(AccountError::KeyNameTaken)));
    // and: the same answer still adds it under another name
    bench
        .accounts
        .finish_adding_key(&ada, "YubiKey green", &answer, &first.session)
        .await
        .expect("the adding is still under way");
}

#[tokio::test]
async fn without_a_public_url_no_key_is_added() {
    // given: an instance not told where it is served
    let wired = wiring::wired();
    let ada = signed_up(&wired, ADA).await;

    // when: adding a key begins
    let begun = wired
        .accounts
        .begin_adding_key(&ada, KeyRole::SecondFactor)
        .await;

    // then: it is refused
    assert!(
        matches!(begun, Err(AccountsError::KeysUnavailable)),
        "{begun:?}"
    );
}

#[tokio::test]
async fn a_key_is_renamed_to_a_name_no_other_key_of_the_account_has() {
    // given: an account holding two keys
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let blue = bench.added(&ada, "YubiKey blue", &session).await;
    let green = bench.added(&ada, "YubiKey green", &blue.session).await;

    // when: one is renamed to a free name
    bench
        .accounts
        .rename_key(&ada, &blue.key, "Keyring")
        .await
        .expect("a free name");

    // then: it holds the new name
    let account = bench
        .wired
        .account(&ada)
        .await
        .expect("the account should exist")
        .state;
    assert_eq!(account.keys()[&blue.key].name.as_str(), "Keyring");
    // and: renaming one to the other's name is refused
    let taken = bench
        .accounts
        .rename_key(&ada, &green.key, "Keyring")
        .await
        .map_err(refused_for);
    assert_eq!(taken, Err(Some(AccountError::KeyNameTaken)));
    // and: a blank name is refused
    let blank = bench.accounts.rename_key(&ada, &green.key, "  ").await;
    assert!(
        matches!(blank, Err(AccountsError::InvalidKeyName(_))),
        "{blank:?}"
    );
}

#[tokio::test]
async fn a_key_is_removed_for_the_password_ending_every_other_session() {
    // given: an account holding a key, signed in twice
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let added = bench.added(&ada, "YubiKey blue", &session).await;
    let elsewhere = bench.session_of(ADA).await;
    assert!(bench.is_open(&elsewhere).await);
    bench
        .wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::seconds(1));

    // when: removing it is asked with a wrong password
    let wrong = bench
        .accounts
        .remove_key(&ada, &added.key, "not her password", &added.session)
        .await;

    // then: it is refused, and the key stays
    assert!(
        matches!(wrong, Err(AccountsError::PasswordWrong)),
        "{wrong:?}"
    );
    // when: it is asked with her password
    let renewed = bench
        .accounts
        .remove_key(&ada, &added.key, HER_PASSWORD, &added.session)
        .await
        .expect("her password removes it");
    // then: the key and its credential are gone, and so is the second step
    let account = bench
        .wired
        .account(&ada)
        .await
        .expect("the account should exist")
        .state;
    assert!(account.keys().is_empty());
    assert!(account.second_factor().is_none());
    assert!(
        bench
            .wired
            .secrets
            .credentials_of(&ada)
            .await
            .expect("reading should succeed")
            .is_empty()
    );
    // and: the session goes on as a new one
    assert!(bench.is_open(&renewed).await);
    assert!(!bench.is_open(&added.session).await);
    assert!(!bench.is_open(&elsewhere).await);
    // and: removing it again is refused, the key being gone
    let again = bench
        .accounts
        .remove_key(&ada, &added.key, HER_PASSWORD, &renewed)
        .await;
    assert!(matches!(again, Err(AccountsError::NoSuchKey)), "{again:?}");
}

#[tokio::test]
async fn guessing_the_password_to_remove_a_key_is_throttled() {
    // given: a signed-in account
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let key = KeyId::generate(clock::at(1_700_000_000));

    // when: five wrong passwords are given
    for _ in 0..5 {
        let wrong = bench
            .accounts
            .remove_key(&ada, &key, "a guess", &session)
            .await;
        assert!(
            matches!(wrong, Err(AccountsError::PasswordWrong)),
            "{wrong:?}"
        );
    }

    // then: the next try has to wait, even with her password
    let waiting = bench
        .accounts
        .remove_key(&ada, &key, HER_PASSWORD, &session)
        .await;
    assert!(
        matches!(waiting, Err(AccountsError::Throttled { .. })),
        "{waiting:?}"
    );
}

async fn with_a_key(bench: &Bench, email: &str) -> (AccountId, SoftPasskey) {
    let account = signed_up(&bench.wired, email).await;
    let session = bench.session_of(email).await;
    let mut authenticator = SoftPasskey::new(true);
    let answer = bench.answered_by(&mut authenticator, &account).await;
    bench
        .accounts
        .finish_adding_key(&account, "YubiKey blue", &answer, &session)
        .await
        .expect("the key is added");

    (account, authenticator)
}

#[tokio::test]
async fn a_session_owing_its_second_factor_is_signed_in_by_a_touch_of_the_key() {
    // given: an account with a key, signing in anew and owing its second factor
    let bench = bench();
    let (ada, mut authenticator) = with_a_key(&bench, ADA).await;
    let owing = bench.session_of(ADA).await;
    let before = bench
        .wired
        .secrets
        .credentials_of(&ada)
        .await
        .expect("reading should succeed");

    // when: the key is touched to answer it
    let touch = bench.touched_by(&mut authenticator, &owing).await;
    let (signed_in, account) = bench
        .accounts
        .answer_with_key(&owing, &touch)
        .await
        .expect("the touch answers the second step");

    // then: a session signed in all the way opens, in place of the owing one
    assert_eq!(account.session.stage, Stage::SignedIn);
    assert!(bench.is_open(&signed_in).await);
    assert!(!bench.is_open(&owing).await);
    // and: the key's credential is written anew, its counter moved on
    let after = bench
        .wired
        .secrets
        .credentials_of(&ada)
        .await
        .expect("reading should succeed");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].0, before[0].0);
    assert_ne!(after[0].1.written, before[0].1.written);
}

#[tokio::test]
async fn a_touch_answers_once() {
    // given: a session owing its second factor, answered by a touch
    let bench = bench();
    let (_, mut authenticator) = with_a_key(&bench, ADA).await;
    let owing = bench.session_of(ADA).await;
    let touch = bench.touched_by(&mut authenticator, &owing).await;
    let other_owing = bench.session_of(ADA).await;
    bench
        .accounts
        .answer_with_key(&owing, &touch)
        .await
        .expect("the touch answers the second step");

    // when: the same touch is given again, for another session owing
    let again = bench.accounts.answer_with_key(&other_owing, &touch).await;

    // then: it is refused, nothing being asked of that session
    assert!(
        matches!(again, Err(AccountsError::NoKeyBeingAsked)),
        "{again:?}"
    );
}

#[tokio::test]
async fn a_touch_given_for_another_accounts_asking_answers_nothing() {
    // given: two accounts with keys, both owing their second factor
    let bench = bench();
    let (_, _) = with_a_key(&bench, ADA).await;
    let (_, mut theirs) = with_a_key(&bench, GRACE).await;
    let hers = bench.session_of(ADA).await;
    let their_owing = bench.session_of(GRACE).await;
    bench
        .accounts
        .begin_answering_with_key(&hers)
        .await
        .expect("answering begins");

    // when: the second account's touch is given for the first
    let touch = bench.touched_by(&mut theirs, &their_owing).await;
    let crossed = bench.accounts.answer_with_key(&hers, &touch).await;

    // then: it is refused
    assert!(
        matches!(crossed, Err(AccountsError::KeyRefused)),
        "{crossed:?}"
    );
}

#[tokio::test]
async fn a_session_owing_nothing_is_not_asked_for_a_key() {
    // given: an account without a second factor, signed in all the way
    let bench = bench();
    signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;

    // when: answering with a key is begun for that session
    let begun = bench.accounts.begin_answering_with_key(&session).await;

    // then: it is refused, nothing being owed
    assert!(
        matches!(begun, Err(AccountsError::NoSecondFactorOwed)),
        "{begun:?}"
    );
}

fn with_totp_too(bench: Bench) -> Bench {
    Bench {
        accounts: bench.accounts.sealing_totp_secrets(
            accounts_core::TotpKey::from_hex(crate::totp_bench::KEY).expect("a test key"),
        ),
        wired: bench.wired,
    }
}

async fn totp_set_up(bench: &Bench, account: &AccountId, session: &SessionToken) -> SessionToken {
    totp_set_up_with_secret(bench, account, session).await.0
}

async fn totp_set_up_with_secret(
    bench: &Bench,
    account: &AccountId,
    session: &SessionToken,
) -> (SessionToken, String) {
    let setting_up = bench
        .accounts
        .begin_setting_up_totp(account)
        .await
        .expect("setting up begins");
    let code = crate::totp_bench::code_of(&setting_up.secret, 1_700_000_000);
    let session = bench
        .accounts
        .finish_setting_up_totp(account, &code, session)
        .await
        .expect("the code finishes setting up")
        .0;

    (session, setting_up.secret)
}

async fn codes_kept(bench: &Bench, account: &AccountId) -> usize {
    bench
        .wired
        .secrets
        .recovery_codes_of(account)
        .await
        .expect("reading should succeed")
        .len()
}

#[tokio::test]
async fn turning_totp_off_beside_a_key_keeps_the_recovery_codes() {
    // given: an account with TOTP and a key answering its second step
    let bench = with_totp_too(bench());
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let session = totp_set_up(&bench, &ada, &session).await;
    let added = bench.added(&ada, "YubiKey blue", &session).await;

    // when: TOTP is turned off for her password
    bench
        .accounts
        .turn_off_totp(&ada, HER_PASSWORD, &added.session)
        .await
        .expect("her password turns it off");

    // then: the recovery codes stay, the key still answering the second step
    assert_eq!(codes_kept(&bench, &ada).await, 10);
    let account = bench
        .wired
        .account(&ada)
        .await
        .expect("the account should exist")
        .state;
    assert_eq!(
        account
            .second_factor()
            .map(|factor| factor.recovery_codes_left),
        Some(10)
    );
}

#[tokio::test]
async fn removing_a_key_keeps_the_codes_while_totp_answers_and_forgets_them_with_the_last() {
    // given: an account with TOTP and a key answering its second step
    let bench = with_totp_too(bench());
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let session = totp_set_up(&bench, &ada, &session).await;
    let added = bench.added(&ada, "YubiKey blue", &session).await;

    // when: the key is removed
    let session = bench
        .accounts
        .remove_key(&ada, &added.key, HER_PASSWORD, &added.session)
        .await
        .expect("her password removes it");

    // then: the codes stay, TOTP still answering
    assert_eq!(codes_kept(&bench, &ada).await, 10);
    // when: TOTP is turned off as well
    bench
        .accounts
        .turn_off_totp(&ada, HER_PASSWORD, &session)
        .await
        .expect("her password turns it off");
    // then: the codes are forgotten with the last second factor
    assert_eq!(codes_kept(&bench, &ada).await, 0);
}

#[tokio::test]
async fn new_recovery_codes_are_drawn_for_the_password_where_only_a_key_answers() {
    // given: an account whose second step only a key answers
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    bench.added(&ada, "YubiKey blue", &session).await;
    let before = bench
        .wired
        .secrets
        .recovery_codes_of(&ada)
        .await
        .expect("reading should succeed");

    // when: new codes are drawn with a wrong password
    let wrong = bench
        .accounts
        .draw_recovery_codes(&ada, "not her password")
        .await;

    // then: it is refused
    assert!(
        matches!(wrong, Err(AccountsError::PasswordWrong)),
        "{wrong:?}"
    );
    // when: they are drawn with her password
    let drawn = bench
        .accounts
        .draw_recovery_codes(&ada, HER_PASSWORD)
        .await
        .expect("her password draws them");
    // then: ten new codes replace the old
    assert_eq!(drawn.len(), 10);
    assert_ne!(
        bench
            .wired
            .secrets
            .recovery_codes_of(&ada)
            .await
            .expect("reading should succeed"),
        before
    );
}

#[tokio::test]
async fn the_way_that_answered_last_is_asked_first_next_time() {
    // given: an account with TOTP and a key
    let bench = with_totp_too(bench());
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let (session, secret) = totp_set_up_with_secret(&bench, &ada, &session).await;
    let mut authenticator = SoftPasskey::new(true);
    let answer = bench.answered_by(&mut authenticator, &ada).await;
    bench
        .accounts
        .finish_adding_key(&ada, "YubiKey blue", &answer, &session)
        .await
        .expect("the key is added");
    let asked_way = async || {
        let account = bench
            .wired
            .account(&ada)
            .await
            .expect("the account should exist")
            .state;
        bench
            .accounts
            .asked_way(&ada, &account)
            .await
            .expect("reading should succeed")
    };

    // expect: a key is asked first, nothing having answered yet
    assert_eq!(asked_way().await, Some(SecondStepWay::Key));
    // when: a sign-in's second step is answered by the app
    bench
        .wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::seconds(30));
    let owing = bench.session_of(ADA).await;
    bench
        .accounts
        .answer_with_totp(&owing, &crate::totp_bench::code_of(&secret, 1_700_000_030))
        .await
        .expect("the app's code answers");
    // then: the app is asked first next time
    assert_eq!(asked_way().await, Some(SecondStepWay::Totp));
    // when: the next one is answered by the key
    let owing = bench.session_of(ADA).await;
    let touch = bench.touched_by(&mut authenticator, &owing).await;
    bench
        .accounts
        .answer_with_key(&owing, &touch)
        .await
        .expect("the touch answers");
    // then: the key is asked first again
    assert_eq!(asked_way().await, Some(SecondStepWay::Key));
}

#[tokio::test]
async fn a_passkey_is_added_to_sign_in_alone_bringing_no_second_step() {
    // given: a signed-in account without a second factor
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;

    // when: a passkey the authenticator made is added
    let asked = bench
        .accounts
        .begin_adding_key(&ada, KeyRole::Primary)
        .await
        .expect("adding begins");
    let answer = made_by(&mut SoftPasskey::new(true), not_resident(asked));
    let added = bench
        .accounts
        .finish_adding_key(&ada, "MacBook", &answer, &session)
        .await
        .expect("the passkey is added");

    // then: the account holds it as signing in alone
    let account = bench
        .wired
        .account(&ada)
        .await
        .expect("the account should exist")
        .state;
    assert_eq!(account.keys()[&added.key].role, KeyRole::Primary);
    // and: it brings no second step, nor recovery codes
    assert!(account.second_factor().is_none());
    assert!(added.recovery_codes.is_none());
    assert_eq!(codes_kept(&bench, &ada).await, 0);
}

async fn with_a_passkey(bench: &Bench, email: &str) -> (AccountId, SoftPasskey) {
    let account = signed_up(&bench.wired, email).await;
    let session = bench.session_of(email).await;
    let mut authenticator = SoftPasskey::new(true);
    let asked = bench
        .accounts
        .begin_adding_key(&account, KeyRole::Primary)
        .await
        .expect("adding begins");
    let answer = made_by(&mut authenticator, not_resident(asked));
    bench
        .accounts
        .finish_adding_key(&account, "MacBook", &answer, &session)
        .await
        .expect("the passkey is added");

    (account, authenticator)
}

/// The software passkey keeps no credential, so the account's credentials are named to it and its answer is given the user handle, as a passkey kept on a device finds and answers them.
async fn passkey_touch(
    bench: &Bench,
    authenticator: &mut SoftPasskey,
    account: &AccountId,
) -> (accounts_core::SigningIn, Value) {
    let (signing_in, mut asked) = bench
        .accounts
        .begin_signing_in_with_passkey()
        .expect("signing in begins");
    let held: Vec<Value> = bench
        .wired
        .secrets
        .credentials_of(account)
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|(_, credential)| {
            let written: Value =
                serde_json::from_str(&credential.written).expect("a credential is JSON");
            json!({ "type": "public-key", "id": written["cred_id"] })
        })
        .collect();
    asked["publicKey"]["allowCredentials"] = held.into();
    let mut touch = touch_of(authenticator, asked);
    touch["response"]["userHandle"] = base64_url(account.as_uuid().as_bytes()).into();

    (signing_in, touch)
}

/// The user handle a passkey kept on its device answers with, unsigned: the account's id, as the server gave it when the passkey was made.
pub(crate) fn base64_url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    bytes
        .chunks(3)
        .flat_map(|chunk| {
            let joined = chunk.iter().enumerate().fold(0_u32, |joined, (at, byte)| {
                joined | u32::from(*byte) << (16 - 8 * at)
            });
            (0..=chunk.len())
                .map(move |at| ALPHABET[(joined >> (18 - 6 * at) & 63) as usize] as char)
        })
        .collect()
}

#[tokio::test]
async fn a_passkey_signs_in_alone_owing_no_second_step() {
    // given: an account with a passkey and a second-step key, nobody signed in
    let bench = bench();
    let (ada, mut authenticator) = with_a_passkey(&bench, ADA).await;
    let session = bench.session_of(ADA).await;
    bench.added(&ada, "YubiKey blue", &session).await;

    // when: the passkey is touched to sign in, no address typed
    let (signing_in, touch) = passkey_touch(&bench, &mut authenticator, &ada).await;
    let (token, signed_in) = bench
        .accounts
        .sign_in_with_passkey(&signing_in, &touch)
        .await
        .expect("the passkey signs in");

    // then: she is signed in all the way, owing no second step though she has one
    assert_eq!(signed_in.session.account, ada);
    assert_eq!(signed_in.session.stage, Stage::SignedIn);
    assert!(bench.is_open(&token).await);
    // and: the same touch signs in nobody again
    let again = bench
        .accounts
        .sign_in_with_passkey(&signing_in, &touch)
        .await;
    assert!(
        matches!(again, Err(AccountsError::NoKeyBeingAsked)),
        "{again:?}"
    );
}

#[tokio::test]
async fn a_key_for_the_second_step_signs_in_nobody_alone() {
    // given: an account whose only key answers the second step
    let bench = bench();
    let ada = signed_up(&bench.wired, ADA).await;
    let session = bench.session_of(ADA).await;
    let mut authenticator = SoftPasskey::new(true);
    let answer = bench.answered_by(&mut authenticator, &ada).await;
    bench
        .accounts
        .finish_adding_key(&ada, "YubiKey blue", &answer, &session)
        .await
        .expect("the key is added");

    // when: it is touched to sign in alone
    let (signing_in, touch) = passkey_touch(&bench, &mut authenticator, &ada).await;
    let refused = bench
        .accounts
        .sign_in_with_passkey(&signing_in, &touch)
        .await;

    // then: it is refused
    assert!(
        matches!(refused, Err(AccountsError::KeyRefused)),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_locked_account_is_not_signed_in_by_its_passkey() {
    // given: an account with a passkey, locked by an operator
    let bench = bench();
    let (ada, mut authenticator) = with_a_passkey(&bench, ADA).await;
    let grace = signed_up(&bench.wired, GRACE).await;
    let code = bench
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("nobody operates yet");
    bench
        .accounts
        .become_operator(&grace, code.as_str())
        .await
        .expect("the code drawn makes an operator");
    bench
        .accounts
        .lock(&grace, &ada, "Sent spam.", None)
        .await
        .expect("the operator locks her");

    // when: its passkey is touched to sign in
    let (signing_in, touch) = passkey_touch(&bench, &mut authenticator, &ada).await;
    let refused = bench
        .accounts
        .sign_in_with_passkey(&signing_in, &touch)
        .await;

    // then: it is refused as locked
    assert!(matches!(refused, Err(AccountsError::Locked)), "{refused:?}");
}
