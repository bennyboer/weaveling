use accounts_core::{
    AccountError, AccountId, AccountsCore, AccountsError, EmailAddress, EmailAddresses,
    KeyCeremonies, KeyCredentials, KeyId, KeyRole, SecondFactors, SessionToken,
};
use eventsourcing::ServiceError;
use serde_json::{Value, json};
use time::Duration;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_rs::prelude::{CreationChallengeResponse, Url};

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

impl Bench {
    async fn session_of(&self, email: &str) -> SessionToken {
        self.accounts
            .begin_session(email, HER_PASSWORD)
            .await
            .expect("signing in should succeed")
            .0
    }

    async fn answered(&self, account: &AccountId) -> Value {
        let asked = self
            .accounts
            .begin_adding_key(account)
            .await
            .expect("adding begins");

        made_by(&mut SoftPasskey::new(true), asked)
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
        .begin_adding_key(&ada)
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
    let begun = wired.accounts.begin_adding_key(&ada).await;

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
