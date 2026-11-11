use accounts_contract::{
    AddKeyRequest, AddedKeyDTO, KeyAnswerRequest, KeyChallengeDTO, KeyChallengeRequest, KeyDTO,
    PasskeyChallengeDTO, PasskeySignInRequest, PasswordRequest, PromoteKeyRequest,
    RenameKeyRequest, SignedInDTO, WaysInDTO, key_role, refused, stage,
};
use accounts_core::{AccountId, KeyCeremonies, KeyCredentials, KeyId, SecondFactors};
use axum::http::StatusCode;
use axum::http::header::COOKIE;
use axum_test::TestResponse;
use serde_json::{Value, json};
use time::OffsetDateTime;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse, Url};

use crate::totp_bench::{Served, cookie_set_by, refused_as, served};
use crate::wiring::HER_PASSWORD;

const SERVED_AT: &str = "https://weaveling.example.com";

async fn with_key_ceremonies() -> Served {
    served(|wired| {
        wired
            .accounts
            .clone()
            .holding_key_ceremonies(KeyCeremonies::serving_at(SERVED_AT).expect("a public URL"))
    })
    .await
}

fn made_for(asked: Value) -> Value {
    made_by(&mut SoftPasskey::new(true), asked)
}

fn made_by(authenticator: &mut SoftPasskey, asked: Value) -> Value {
    let asked: CreationChallengeResponse =
        serde_json::from_value(asked).expect("the browser is asked in the WebAuthn shape");
    let made = authenticator
        .do_registration(Url::parse(SERVED_AT).expect("a URL"), asked)
        .expect("the authenticator makes a key");

    serde_json::to_value(made).expect("the answer is plain data")
}

impl Served {
    async fn challenge(&self, cookie: &str) -> TestResponse {
        self.challenge_as(cookie, key_role::SECOND_FACTOR).await
    }

    async fn key_made(&self, cookie: &str) -> Value {
        let response = self.challenge(cookie).await;
        response.assert_status_ok();

        made_for(response.json::<KeyChallengeDTO>().options)
    }

    async fn add(&self, cookie: &str, name: &str, answer: Value) -> TestResponse {
        self.server
            .post("/accounts/current/keys")
            .add_header(COOKIE, cookie.to_owned())
            .json(&AddKeyRequest {
                name: name.to_owned(),
                answer,
            })
            .await
    }

    async fn keys(&self, cookie: &str) -> Vec<KeyDTO> {
        let response = self
            .server
            .get("/accounts/current/keys")
            .add_header(COOKIE, cookie.to_owned())
            .await;
        response.assert_status_ok();

        response.json()
    }

    async fn rename(&self, cookie: &str, key: &str, name: &str) -> TestResponse {
        self.server
            .patch(&format!("/accounts/current/keys/{key}"))
            .add_header(COOKIE, cookie.to_owned())
            .json(&RenameKeyRequest {
                name: name.to_owned(),
            })
            .await
    }

    async fn ways_in_seen_by(&self, cookie: &str, account: &AccountId) -> TestResponse {
        self.server
            .get(&format!("/instance/accounts/{account}/ways-in"))
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    async fn key_removed_by(
        &self,
        cookie: &str,
        account: &AccountId,
        key: &str,
        seen: u64,
    ) -> TestResponse {
        self.server
            .delete(&format!("/instance/accounts/{account}/keys/{key}"))
            .add_header(COOKIE, cookie.to_owned())
            .add_header("if-match", format!("\"{seen}\""))
            .await
    }

    async fn remove(&self, cookie: &str, key: &str, password: &str) -> TestResponse {
        self.server
            .delete(&format!("/accounts/current/keys/{key}"))
            .add_header(COOKIE, cookie.to_owned())
            .json(&PasswordRequest {
                password: password.to_owned(),
            })
            .await
    }
}

#[tokio::test]
async fn a_key_is_added_listed_renamed_and_removed() {
    // given: a signed-in author on an instance that adds keys
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;

    // when: a key the authenticator made is added under a name
    let answer = served.key_made(&cookie).await;
    let added = served.add(&cookie, "YubiKey blue", answer).await;

    // then: it is created, with ten recovery codes, in a session of its own
    added.assert_status(StatusCode::CREATED);
    let cookie = cookie_set_by(&added);
    let key = added.json::<AddedKeyDTO>();
    assert_eq!(key.recovery_codes.map(|codes| codes.len()), Some(10));
    // and: it is listed, answering the second step
    let listed = served.keys(&cookie).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].key, key.key);
    assert_eq!(listed[0].name, "YubiKey blue");
    assert_eq!(listed[0].role, key_role::SECOND_FACTOR);
    // when: it is renamed
    served
        .rename(&cookie, &key.key, "Keyring")
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // then: it is listed under its new name
    assert_eq!(served.keys(&cookie).await[0].name, "Keyring");
    // when: removing it is asked with a wrong password
    let wrong = served.remove(&cookie, &key.key, "not her password").await;
    // then: it is refused, and the key stays
    refused_as(
        &wrong,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::PASSWORD_WRONG,
    );
    assert_eq!(served.keys(&cookie).await.len(), 1);
    // when: it is asked with her password
    let removed = served.remove(&cookie, &key.key, HER_PASSWORD).await;
    // then: it is gone, the session going on as a new one
    removed.assert_status(StatusCode::NO_CONTENT);
    let cookie = cookie_set_by(&removed);
    assert!(served.keys(&cookie).await.is_empty());
}

#[tokio::test]
async fn without_a_public_url_no_key_is_added() {
    // given: a signed-in author on an instance not told where it is served
    let served = crate::totp_bench::with_a_key().await;
    let cookie = served.signed_in().await;

    // when: adding a key is begun
    let begun = served.challenge(&cookie).await;

    // then: it is refused, saying keys are not kept here
    refused_as(&begun, StatusCode::CONFLICT, refused::KEYS_UNAVAILABLE);
}

#[tokio::test]
async fn an_answer_nothing_asked_for_or_that_makes_no_key_is_refused() {
    // given: a signed-in author
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;

    // when: an answer is given with no adding under way
    let unasked = served
        .add(&cookie, "YubiKey blue", json!({ "id": "nonsense" }))
        .await;

    // then: it is refused, the adding to begin again
    refused_as(&unasked, StatusCode::CONFLICT, refused::NO_KEY_BEING_ADDED);
    // when: an answer that is none is given to an adding under way
    served.key_made(&cookie).await;
    let nonsense = served
        .add(&cookie, "YubiKey blue", json!({ "id": "nonsense" }))
        .await;
    // then: it is refused as making no key
    refused_as(
        &nonsense,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::KEY_REFUSED,
    );
}

#[tokio::test]
async fn a_key_name_is_refused_for_what_is_wrong_with_it() {
    // given: a signed-in author holding a key
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let answer = served.key_made(&cookie).await;
    let added = served.add(&cookie, "YubiKey blue", answer).await;
    let cookie = cookie_set_by(&added);
    let key = added.json::<AddedKeyDTO>().key;

    // expect: a blank name is refused
    refused_as(
        &served.rename(&cookie, &key, "  ").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::KEY_NAME_BLANK,
    );
    // and: a name too long is refused, saying the limit
    let long = served.rename(&cookie, &key, &"k".repeat(61)).await;
    refused_as(
        &long,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::KEY_NAME_TOO_LONG,
    );
    assert_eq!(long.json::<Value>()["max"], 60);
    // and: a name another key has is refused
    let answer = served.key_made(&cookie).await;
    refused_as(
        &served.add(&cookie, "YubiKey blue", answer).await,
        StatusCode::CONFLICT,
        refused::KEY_NAME_TAKEN,
    );
}

#[tokio::test]
async fn a_key_the_account_does_not_hold_or_no_key_at_all_is_refused() {
    // given: a signed-in author without keys
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let nobodys = accounts_core::KeyId::generate(clock::at(1_700_000_000)).to_string();

    // expect: renaming a key the account does not hold is refused
    refused_as(
        &served.rename(&cookie, &nobodys, "Keyring").await,
        StatusCode::NOT_FOUND,
        refused::NO_SUCH_KEY,
    );
    // and: removing one is refused alike
    refused_as(
        &served.remove(&cookie, &nobodys, HER_PASSWORD).await,
        StatusCode::NOT_FOUND,
        refused::NO_SUCH_KEY,
    );
    // and: a path naming no key at all is rejected
    refused_as(
        &served.rename(&cookie, "not-a-key", "Keyring").await,
        StatusCode::BAD_REQUEST,
        refused::KEY_ID_INVALID,
    );
}

#[tokio::test]
async fn keys_are_for_the_signed_in_only() {
    // given: an instance that adds keys, and nobody signed in
    let served = with_key_ceremonies().await;

    // expect: listing keys asks to sign in first
    refused_as(
        &served.server.get("/accounts/current/keys").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: so does beginning to add one
    refused_as(
        &served.server.post("/accounts/current/keys/challenge").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

fn touch_of(authenticator: &mut SoftPasskey, asked: Value) -> Value {
    let asked: RequestChallengeResponse =
        serde_json::from_value(asked).expect("the browser is asked in the WebAuthn shape");
    let touched = authenticator
        .do_authentication(Url::parse(SERVED_AT).expect("a URL"), asked)
        .expect("the authenticator signs the challenge");

    serde_json::to_value(touched).expect("the answer is plain data")
}

impl Served {
    async fn asked_for_a_key(&self, owing: &str) -> TestResponse {
        self.server
            .post("/sessions/current/key/challenge")
            .add_header(COOKIE, owing.to_owned())
            .await
    }

    async fn answer_with_key(&self, owing: &str, answer: Value) -> TestResponse {
        self.server
            .post("/sessions/current/key")
            .add_header(COOKIE, owing.to_owned())
            .json(&KeyAnswerRequest { answer })
            .await
    }
}

#[tokio::test]
async fn a_session_owing_its_second_factor_is_signed_in_by_a_touch_of_the_key() {
    // given: an author with a key, signing in anew and owing the second factor
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let mut authenticator = SoftPasskey::new(true);
    let asked = served
        .challenge(&cookie)
        .await
        .json::<KeyChallengeDTO>()
        .options;
    let made = made_by(&mut authenticator, asked);
    served
        .add(&cookie, "YubiKey blue", made)
        .await
        .assert_status(StatusCode::CREATED);
    let owing = served.signed_in().await;

    // when: the key is asked for and touched
    let asked = served.asked_for_a_key(&owing).await;
    asked.assert_status_ok();
    let touch = touch_of(&mut authenticator, asked.json::<KeyChallengeDTO>().options);
    let answered = served.answer_with_key(&owing, touch).await;

    // then: the author is signed in all the way, in a session of its own
    answered.assert_status_ok();
    let signed_in = answered.json::<SignedInDTO>();
    assert_eq!(signed_in.stage, stage::SIGNED_IN);
    assert_ne!(cookie_set_by(&answered), owing);
    // and: the second step is told to be answered by a key, not by an app
    let factor = signed_in.second_factor.expect("a second factor");
    assert!(factor.answered_by_key);
    assert!(!factor.answered_by_totp);
}

#[tokio::test]
async fn a_touch_with_nothing_asked_is_refused_and_nobody_signed_in_is_asked_nothing() {
    // given: an author with a key, owing the second factor, never asked for the key
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let made = served.key_made(&cookie).await;
    served
        .add(&cookie, "YubiKey blue", made)
        .await
        .assert_status(StatusCode::CREATED);
    let owing = served.signed_in().await;

    // when: an answer is given
    let unasked = served
        .answer_with_key(&owing, json!({ "id": "nonsense" }))
        .await;

    // then: it is refused, the asking to begin again
    refused_as(&unasked, StatusCode::CONFLICT, refused::NO_KEY_BEING_ASKED);
    // and: without a session, nothing is asked for
    refused_as(
        &served.server.post("/sessions/current/key/challenge").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

impl Served {
    async fn challenge_as(&self, cookie: &str, role: &str) -> TestResponse {
        self.server
            .post("/accounts/current/keys/challenge")
            .add_header(COOKIE, cookie.to_owned())
            .json(&KeyChallengeRequest {
                role: role.to_owned(),
                replacing: None,
                password: HER_PASSWORD.to_owned(),
            })
            .await
    }
}

#[tokio::test]
async fn a_passkey_is_asked_for_resident_and_checking_its_user_and_listed_as_signing_in_alone() {
    // given: a signed-in author
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;

    // when: adding a passkey begins
    let asked = served.challenge_as(&cookie, key_role::PRIMARY).await;

    // then: the browser is asked for a resident credential that checks its user
    asked.assert_status_ok();
    let options = asked.json::<KeyChallengeDTO>().options;
    let selection = &options["publicKey"]["authenticatorSelection"];
    assert_eq!(selection["residentKey"], "required");
    assert_eq!(selection["userVerification"], "required");
    // when: the passkey made is added
    let mut eased = options;
    eased["publicKey"]["authenticatorSelection"]["residentKey"] = "discouraged".into();
    eased["publicKey"]["authenticatorSelection"]["requireResidentKey"] = false.into();
    let added = served.add(&cookie, "MacBook", made_for(eased)).await;
    // then: it is listed as signing in alone, with no recovery codes
    added.assert_status(StatusCode::CREATED);
    assert!(added.json::<AddedKeyDTO>().recovery_codes.is_none());
    let cookie = cookie_set_by(&added);
    assert_eq!(served.keys(&cookie).await[0].role, key_role::PRIMARY);
}

#[tokio::test]
async fn a_key_asked_for_in_a_role_that_is_none_is_rejected() {
    // given: a signed-in author
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;

    // expect: a role that is none is rejected
    refused_as(
        &served.challenge_as(&cookie, "bodyguard").await,
        StatusCode::BAD_REQUEST,
        refused::KEY_ROLE_UNKNOWN,
    );
}

impl Served {
    async fn passkey_signing_in(&self) -> PasskeyChallengeDTO {
        let asked = self.server.post("/sessions/passkey/challenge").await;
        asked.assert_status_ok();

        asked.json()
    }

    async fn sign_in_with_passkey(&self, signing_in: &str, answer: Value) -> TestResponse {
        self.server
            .post("/sessions/passkey")
            .json(&PasskeySignInRequest {
                signing_in: signing_in.to_owned(),
                answer,
            })
            .await
    }
}

#[tokio::test]
async fn a_passkey_signs_in_alone_under_a_new_cookie_with_no_address_typed() {
    // given: an author with a passkey, made over REST
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let mut authenticator = SoftPasskey::new(true);
    let mut asked = served
        .challenge_as(&cookie, key_role::PRIMARY)
        .await
        .json::<KeyChallengeDTO>()
        .options;
    asked["publicKey"]["authenticatorSelection"]["residentKey"] = "discouraged".into();
    asked["publicKey"]["authenticatorSelection"]["requireResidentKey"] = false.into();
    let made = made_by(&mut authenticator, asked);
    let credential = made["rawId"].clone();
    served
        .add(&cookie, "MacBook", made)
        .await
        .assert_status(StatusCode::CREATED);

    // when: signing in by passkey is begun by nobody, and the passkey touched
    let signing_in = served.passkey_signing_in().await;
    let mut options = signing_in.options;
    assert!(
        options.get("mediation").is_none(),
        "a button asks too, not only autofill"
    );
    options["publicKey"]["allowCredentials"] = json!([{ "type": "public-key", "id": credential }]);
    let mut touch = touch_of(&mut authenticator, options);
    touch["response"]["userHandle"] =
        crate::key_service::base64_url(served.ada.as_uuid().as_bytes()).into();
    let signed_in = served
        .sign_in_with_passkey(&signing_in.signing_in, touch.clone())
        .await;

    // then: she is signed in all the way, under a cookie of her own
    signed_in.assert_status_ok();
    assert_eq!(signed_in.json::<SignedInDTO>().stage, stage::SIGNED_IN);
    let renewed = cookie_set_by(&signed_in);
    assert_eq!(served.keys(&renewed).await.len(), 1);
    // and: the same touch signs in nobody again
    refused_as(
        &served
            .sign_in_with_passkey(&signing_in.signing_in, touch)
            .await,
        StatusCode::CONFLICT,
        refused::NO_KEY_BEING_ASKED,
    );
}

#[tokio::test]
async fn a_key_is_promoted_by_a_passkey_made_in_its_place_and_demoted_again_for_the_password() {
    // given: an author with a key answering the second step
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let made = served.key_made(&cookie).await;
    let added = served.add(&cookie, "YubiKey blue", made).await;
    let cookie = cookie_set_by(&added);
    let key = added.json::<AddedKeyDTO>().key;

    // when: a passkey is asked for in its place and made
    let mut asked = served
        .server
        .post("/accounts/current/keys/challenge")
        .add_header(COOKIE, cookie.clone())
        .json(&KeyChallengeRequest {
            role: key_role::PRIMARY.to_owned(),
            replacing: Some(key.clone()),
            password: HER_PASSWORD.to_owned(),
        })
        .await
        .json::<KeyChallengeDTO>()
        .options;
    asked["publicKey"]["authenticatorSelection"]["residentKey"] = "discouraged".into();
    asked["publicKey"]["authenticatorSelection"]["requireResidentKey"] = false.into();
    let promoted = served
        .server
        .post(&format!("/accounts/current/keys/{key}/promote"))
        .add_header(COOKIE, cookie.clone())
        .json(&PromoteKeyRequest {
            answer: made_for(asked),
            password: HER_PASSWORD.to_owned(),
        })
        .await;

    // then: one key is listed, under the old name, signing in alone
    promoted.assert_status_ok();
    let cookie = cookie_set_by(&promoted);
    let listed = served.keys(&cookie).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "YubiKey blue");
    assert_eq!(listed[0].role, key_role::PRIMARY);
    // when: it is demoted again with her password
    let demoted = served
        .server
        .post(&format!("/accounts/current/keys/{}/demote", listed[0].key))
        .add_header(COOKIE, cookie.clone())
        .json(&PasswordRequest {
            password: HER_PASSWORD.to_owned(),
        })
        .await;
    // then: it answers the second step, the first second factor once more, with ten codes
    demoted.assert_status_ok();
    assert_eq!(
        demoted
            .json::<AddedKeyDTO>()
            .recovery_codes
            .map(|codes| codes.len()),
        Some(10)
    );
    let cookie = cookie_set_by(&demoted);
    assert_eq!(served.keys(&cookie).await[0].role, key_role::SECOND_FACTOR);
}

#[tokio::test]
async fn the_operator_sees_how_an_account_signs_in_and_removes_a_lost_key_ending_its_sessions() {
    // given: Ada with a key answering her second step, signed in, and Grace the operator
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let answer = served.key_made(&cookie).await;
    let added = served.add(&cookie, "YubiKey blue", answer).await;
    let cookie = cookie_set_by(&added);
    let key = added.json::<AddedKeyDTO>().key;
    let (grace, operating) = served.grace_the_operator().await;

    // when: Grace looks at how Ada signs in
    let seen = served.ways_in_seen_by(&operating, &served.ada).await;

    // then: she sees the key by name and role, and the recovery codes left, no app
    seen.assert_status_ok();
    let ways = seen.json::<WaysInDTO>();
    assert_eq!(ways.keys.len(), 1);
    assert_eq!(ways.keys[0].key, key);
    assert_eq!(ways.keys[0].name, "YubiKey blue");
    assert_eq!(ways.keys[0].role, key_role::SECOND_FACTOR);
    assert_eq!(ways.totp_since, None);
    assert_eq!(ways.recovery_codes_left, Some(10));
    // when: she removes the key a minute later, from the version she saw
    served.at(1_700_000_060);
    let removed = served
        .key_removed_by(&operating, &served.ada, &key, ways.version)
        .await;

    // then: it is gone, and with it the second step and its recovery codes
    removed.assert_status(StatusCode::NO_CONTENT);
    let ways = served
        .ways_in_seen_by(&operating, &served.ada)
        .await
        .json::<WaysInDTO>();
    assert!(ways.keys.is_empty());
    assert_eq!(ways.recovery_codes_left, None);
    assert!(
        served
            .wired
            .secrets
            .credentials_of(&served.ada)
            .await
            .expect("reading should succeed")
            .is_empty()
    );
    assert!(
        served
            .wired
            .secrets
            .recovery_codes_of(&served.ada)
            .await
            .expect("reading should succeed")
            .is_empty()
    );
    // and: Ada is signed out, and signs in by her password alone
    refused_as(
        &served.current(&cookie).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    let signed_in = served.signed_in().await;
    assert_eq!(served.stage_of(&signed_in).await, stage::SIGNED_IN);
    // and: Grace's own session goes on
    served
        .ways_in_seen_by(&operating, &grace)
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn the_operator_removing_one_of_two_keys_leaves_the_second_step_and_its_codes() {
    // given: Ada with two keys answering her second step, and Grace the operator
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let answer = served.key_made(&cookie).await;
    let first = served.add(&cookie, "YubiKey blue", answer).await;
    let cookie = cookie_set_by(&first);
    let first = first.json::<AddedKeyDTO>().key;
    let answer = served.key_made(&cookie).await;
    served.add(&cookie, "YubiKey green", answer).await;
    let (_, operating) = served.grace_the_operator().await;
    let seen = served
        .ways_in_seen_by(&operating, &served.ada)
        .await
        .json::<WaysInDTO>();

    // when: Grace removes the first
    served
        .key_removed_by(&operating, &served.ada, &first, seen.version)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: the second still answers the second step, the codes kept
    let ways = served
        .ways_in_seen_by(&operating, &served.ada)
        .await
        .json::<WaysInDTO>();
    assert_eq!(
        ways.keys
            .iter()
            .map(|key| key.name.as_str())
            .collect::<Vec<_>>(),
        ["YubiKey green"]
    );
    assert_eq!(ways.recovery_codes_left, Some(10));
}

#[tokio::test]
async fn the_operator_is_refused_a_key_not_held_an_account_unknown_and_a_version_gone_by() {
    // given: Ada with a key, and Grace the operator who saw her ways in
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let answer = served.key_made(&cookie).await;
    let added = served.add(&cookie, "YubiKey blue", answer).await;
    let key = added.json::<AddedKeyDTO>().key;
    let (_, operating) = served.grace_the_operator().await;
    let seen = served
        .ways_in_seen_by(&operating, &served.ada)
        .await
        .json::<WaysInDTO>();

    // expect: a key Ada does not hold is not found
    refused_as(
        &served
            .key_removed_by(
                &operating,
                &served.ada,
                &KeyId::generate(OffsetDateTime::UNIX_EPOCH).to_string(),
                seen.version,
            )
            .await,
        StatusCode::NOT_FOUND,
        refused::NO_SUCH_KEY,
    );
    // and: an account nobody signed up with is not found
    refused_as(
        &served
            .ways_in_seen_by(&operating, &AccountId::generate(OffsetDateTime::UNIX_EPOCH))
            .await,
        StatusCode::NOT_FOUND,
        refused::ACCOUNT_NOT_FOUND,
    );
    // and: removing from a version gone by is refused, the key kept
    refused_as(
        &served
            .key_removed_by(&operating, &served.ada, &key, seen.version - 1)
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );
    assert_eq!(
        served
            .ways_in_seen_by(&operating, &served.ada)
            .await
            .json::<WaysInDTO>()
            .keys
            .len(),
        1
    );
}

#[tokio::test]
async fn only_an_operator_sees_how_another_account_signs_in_or_removes_its_keys() {
    // given: Ada with a key, and Grace who is no operator
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;
    let answer = served.key_made(&cookie).await;
    let added = served.add(&cookie, "YubiKey blue", answer).await;
    let cookie = cookie_set_by(&added);
    let key = added.json::<AddedKeyDTO>().key;

    // expect: Ada herself, no operator, is refused both
    refused_as(
        &served.ways_in_seen_by(&cookie, &served.ada).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    refused_as(
        &served.key_removed_by(&cookie, &served.ada, &key, 1).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    // and: the key stays
    assert_eq!(served.keys(&cookie).await.len(), 1);
}

#[tokio::test]
async fn adding_a_key_asks_for_her_password_first_and_a_wrong_one_asks_the_browser_for_nothing() {
    // given: Ada signed in, on an instance that keeps keys
    let served = with_key_ceremonies().await;
    let cookie = served.signed_in().await;

    for role in [key_role::PRIMARY, key_role::SECOND_FACTOR] {
        // when: she begins adding a key with a wrong password
        let refused = served
            .server
            .post("/accounts/current/keys/challenge")
            .add_header(COOKIE, cookie.clone())
            .json(&KeyChallengeRequest {
                role: role.to_owned(),
                replacing: None,
                password: "not her password".to_owned(),
            })
            .await;

        // then: it is refused as the wrong password, and no key is asked for
        refused_as(
            &refused,
            StatusCode::UNPROCESSABLE_ENTITY,
            refused::PASSWORD_WRONG,
        );
    }
}
