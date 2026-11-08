use accounts_contract::{
    AddKeyRequest, AddedKeyDTO, AddingKeyDTO, KeyDTO, PasswordRequest, RenameKeyRequest, key_role,
    refused,
};
use accounts_core::KeyCeremonies;
use axum::http::StatusCode;
use axum::http::header::COOKIE;
use axum_test::TestResponse;
use serde_json::{Value, json};
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_rs::prelude::{CreationChallengeResponse, Url};

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
    let asked: CreationChallengeResponse =
        serde_json::from_value(asked).expect("the browser is asked in the WebAuthn shape");
    let made = SoftPasskey::new(true)
        .do_registration(Url::parse(SERVED_AT).expect("a URL"), asked)
        .expect("the authenticator makes a key");

    serde_json::to_value(made).expect("the answer is plain data")
}

impl Served {
    async fn challenge(&self, cookie: &str) -> TestResponse {
        self.server
            .post("/accounts/current/keys/challenge")
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    async fn key_made(&self, cookie: &str) -> Value {
        let response = self.challenge(cookie).await;
        response.assert_status_ok();

        made_for(response.json::<AddingKeyDTO>().options)
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
