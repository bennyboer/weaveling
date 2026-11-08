use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use webauthn_rs::prelude::{
    Credential, CredentialID, PublicKeyCredential, RegisterPublicKeyCredential, SecurityKey,
    SecurityKeyAuthentication, SecurityKeyRegistration, Url,
};
use webauthn_rs::{Webauthn, WebauthnBuilder};

use crate::id::AccountId;
use crate::key::KeyId;
use crate::secrets::KeyCredential;

/// Adding keys and passkeys and signing in with them, for this instance: every key is bound to the domain it is served at, so moving it voids every key.
#[derive(Debug, Clone)]
pub struct KeyCeremonies {
    webauthn: Webauthn,
    adding: Arc<UnderWay<SecurityKeyRegistration>>,
    answering: Arc<UnderWay<SecurityKeyAuthentication>>,
}

/// What the server keeps of each account's ceremony between asking the browser and hearing back: never written down, so a restart only asks for it to begin again.
#[derive(Debug)]
struct UnderWay<S>(Mutex<HashMap<AccountId, (S, OffsetDateTime)>>);

/// Why the browser's answer does not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CeremonyFailed {
    #[error("nothing is under way for this account; begin again")]
    NothingUnderWay,
    #[error("this answer is no key's for this instance")]
    Refused,
}

impl<S> Default for UnderWay<S> {
    fn default() -> Self {
        Self(Mutex::default())
    }
}

impl<S> UnderWay<S> {
    /// Holds a ceremony begun, in place of the account's one before; the oldest gives way when too many are under way.
    fn hold(&self, account: &AccountId, state: S, now: OffsetDateTime) {
        let mut held = self.0.lock().expect("the ceremonies are not poisoned");
        held.retain(|_, (_, begun)| now - *begun < KeyCeremonies::LASTS);
        if held.len() >= KeyCeremonies::MOST_UNDER_WAY
            && let Some(oldest) = held
                .iter()
                .min_by_key(|(_, (_, begun))| *begun)
                .map(|(account, _)| *account)
        {
            held.remove(&oldest);
        }
        held.insert(*account, (state, now));
    }

    /// The account's ceremony, once, if it is still in time.
    fn take(&self, account: &AccountId, now: OffsetDateTime) -> Result<S, CeremonyFailed> {
        self.0
            .lock()
            .expect("the ceremonies are not poisoned")
            .remove(account)
            .filter(|(_, begun)| now - *begun < KeyCeremonies::LASTS)
            .map(|(state, _)| state)
            .ok_or(CeremonyFailed::NothingUnderWay)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidPublicUrl {
    #[error("it is no address, such as https://weaveling.example.com")]
    NoUrl,
    #[error("browsers make keys only over https, or over http at localhost")]
    NotSecure,
    #[error("keys are bound to a domain name, not to an IP address")]
    NoDomain,
    #[error(
        "it names more than where the instance is served: a path, a query, a fragment or credentials"
    )]
    MoreThanTheOrigin,
}

impl KeyCeremonies {
    const NAME: &str = "Weaveling";
    const LASTS: Duration = Duration::minutes(5);
    const MOST_UNDER_WAY: usize = 1_000;

    /// The ceremonies of an instance served at this address, which is where the browser shows it, not where the server listens.
    pub fn serving_at(public_url: &str) -> Result<Self, InvalidPublicUrl> {
        let url = Url::parse(public_url.trim()).map_err(|_| InvalidPublicUrl::NoUrl)?;
        if !url.has_host() {
            return Err(InvalidPublicUrl::NoUrl);
        }
        let domain = url.domain().ok_or(InvalidPublicUrl::NoDomain)?.to_owned();
        match url.scheme() {
            "https" => {}
            "http" if domain == "localhost" => {}
            "http" => return Err(InvalidPublicUrl::NotSecure),
            _ => return Err(InvalidPublicUrl::NoUrl),
        }
        if url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(InvalidPublicUrl::MoreThanTheOrigin);
        }

        WebauthnBuilder::new(&domain, &url)
            .and_then(|builder| builder.rp_name(Self::NAME).build())
            .map(|webauthn| Self {
                webauthn,
                adding: Arc::default(),
                answering: Arc::default(),
            })
            .map_err(|_| InvalidPublicUrl::NoUrl)
    }

    /// Asks the browser to make a key answering the account's second step, none of the keys it holds already; a begun adding replaces the one before. Not asking for credProtect, which browsers refuse for a key that is not resident and only prefers checking its user, and which a second step after the password does not need.
    pub(crate) fn begin_adding(
        &self,
        account: &AccountId,
        user_name: &str,
        display_name: &str,
        held: &[KeyCredential],
        now: OffsetDateTime,
    ) -> Value {
        let exclude = held
            .iter()
            .map(|credential| CredentialID::from(credential.id.clone()))
            .collect();
        let (mut asked, state) = self
            .webauthn
            .start_securitykey_registration(
                account.as_uuid(),
                user_name,
                display_name,
                Some(exclude),
                None,
                None,
            )
            .expect("a security key registration begins for any account");
        if let Some(extensions) = asked.public_key.extensions.as_mut() {
            extensions.cred_protect = None;
        }
        self.adding.hold(account, state, now);

        serde_json::to_value(asked).expect("what a browser is asked is plain data")
    }

    /// Finishes the account's adding with the browser's answer, once: the credential to keep, or why there is none.
    pub(crate) fn finish_adding(
        &self,
        account: &AccountId,
        answer: &Value,
        now: OffsetDateTime,
    ) -> Result<KeyCredential, CeremonyFailed> {
        let state = self.adding.take(account, now)?;
        let answer: RegisterPublicKeyCredential =
            serde_json::from_value(answer.clone()).map_err(|_| CeremonyFailed::Refused)?;
        let key = self
            .webauthn
            .finish_securitykey_registration(&answer, &state)
            .map_err(|_| CeremonyFailed::Refused)?;
        let credential = Credential::from(key);

        Ok(KeyCredential {
            id: credential.cred_id.to_vec(),
            written: serde_json::to_string(&credential).expect("a credential is plain data"),
        })
    }

    /// Asks the browser for a touch of one of the keys held, to answer the account's second step.
    pub(crate) fn begin_answering(
        &self,
        account: &AccountId,
        held: &[(KeyId, KeyCredential)],
        now: OffsetDateTime,
    ) -> Result<Value, CeremonyFailed> {
        let keys: Vec<SecurityKey> = held
            .iter()
            .filter_map(|(_, credential)| read(credential))
            .map(SecurityKey::from)
            .collect();
        let (asked, state) = self
            .webauthn
            .start_securitykey_authentication(&keys)
            .map_err(|_| CeremonyFailed::Refused)?;
        self.answering.hold(account, state, now);

        Ok(serde_json::to_value(asked).expect("what a browser is asked is plain data"))
    }

    /// Finishes the account's answering with the browser's answer, once: the key that answered, and its credential as it is now, its counter moved on.
    pub(crate) fn finish_answering(
        &self,
        account: &AccountId,
        answer: &Value,
        held: &[(KeyId, KeyCredential)],
        now: OffsetDateTime,
    ) -> Result<(KeyId, String), CeremonyFailed> {
        let state = self.answering.take(account, now)?;
        let answer: PublicKeyCredential =
            serde_json::from_value(answer.clone()).map_err(|_| CeremonyFailed::Refused)?;
        let answered = self
            .webauthn
            .finish_securitykey_authentication(&answer, &state)
            .map_err(|_| CeremonyFailed::Refused)?;
        let (key, credential) = held
            .iter()
            .find(|(_, credential)| credential.id.as_slice() == answered.cred_id().as_slice())
            .and_then(|(key, credential)| Some((*key, read(credential)?)))
            .ok_or(CeremonyFailed::Refused)?;
        let mut renewed = SecurityKey::from(credential);
        renewed.update_credential(&answered);

        Ok((
            key,
            serde_json::to_string(&Credential::from(renewed)).expect("a credential is plain data"),
        ))
    }
}

fn read(credential: &KeyCredential) -> Option<Credential> {
    serde_json::from_str(&credential.written).ok()
}

#[cfg(test)]
mod tests {
    use webauthn_rs::prelude::Uuid;

    use super::*;

    fn bound_and_shown_as(ceremonies: &KeyCeremonies) -> (String, String) {
        let (challenge, _) = ceremonies
            .webauthn
            .start_passkey_registration(Uuid::nil(), "ada", "Ada", None)
            .expect("a registration can begin");

        (challenge.public_key.rp.id, challenge.public_key.rp.name)
    }

    #[test]
    fn an_instance_served_over_https_binds_keys_to_its_domain() {
        // given: an instance served over https
        let ceremonies =
            KeyCeremonies::serving_at("https://weaveling.example.com").expect("ceremonies");

        // when: a key is about to be made
        let (id, name) = bound_and_shown_as(&ceremonies);

        // then: the key is bound to the domain and shown as Weaveling
        assert_eq!(id, "weaveling.example.com");
        assert_eq!(name, "Weaveling");
    }

    #[test]
    fn an_instance_at_localhost_may_be_served_over_http_and_at_any_port() {
        // given: an instance on the author's own machine
        let ceremonies =
            KeyCeremonies::serving_at(" http://localhost:8080/ \n").expect("ceremonies");

        // when: a key is about to be made
        let (id, _) = bound_and_shown_as(&ceremonies);

        // then: the key is bound to localhost
        assert_eq!(id, "localhost");
    }

    #[test]
    fn an_address_keys_cannot_be_bound_to_is_refused_for_what_is_wrong_with_it() {
        // given: addresses browsers would make no key for, each with its reason
        let refused = [
            ("weaveling.example.com", InvalidPublicUrl::NoUrl),
            ("ftp://weaveling.example.com", InvalidPublicUrl::NoUrl),
            ("http://weaveling.example.com", InvalidPublicUrl::NotSecure),
            ("https://127.0.0.1:8080", InvalidPublicUrl::NoDomain),
            ("http://[::1]:8080", InvalidPublicUrl::NoDomain),
            (
                "https://weaveling.example.com/books",
                InvalidPublicUrl::MoreThanTheOrigin,
            ),
            (
                "https://weaveling.example.com/?at=home",
                InvalidPublicUrl::MoreThanTheOrigin,
            ),
            (
                "https://weaveling.example.com/#top",
                InvalidPublicUrl::MoreThanTheOrigin,
            ),
            (
                "https://ada@weaveling.example.com",
                InvalidPublicUrl::MoreThanTheOrigin,
            ),
        ];

        for (address, reason) in refused {
            // when: the ceremonies are asked for at the address
            let ceremonies = KeyCeremonies::serving_at(address);

            // then: it is refused for that reason
            assert_eq!(ceremonies.err(), Some(reason), "{address}");
        }
    }

    #[test]
    fn a_key_for_the_second_step_is_asked_for_without_credprotect() {
        // given: an instance making keys
        let ceremonies =
            KeyCeremonies::serving_at("https://weaveling.example.com").expect("ceremonies");
        let account = AccountId::generate(clock::at(1_000));

        // when: adding a key begins
        let asked =
            ceremonies.begin_adding(&account, "ada@example.com", "Ada", &[], clock::at(1_000));

        // then: the browser is asked for no credential protection, which it would refuse
        let extensions = &asked["publicKey"]["extensions"];
        assert!(
            extensions.get("credentialProtectionPolicy").is_none(),
            "{extensions}"
        );
        assert!(
            extensions
                .get("enforceCredentialProtectionPolicy")
                .is_none(),
            "{extensions}"
        );
    }
}
