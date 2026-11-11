use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use webauthn_rs::prelude::{
    Credential, CredentialID, DiscoverableAuthentication, Passkey, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, SecurityKey, SecurityKeyAuthentication,
    SecurityKeyRegistration, Url,
};
use webauthn_rs::{Webauthn, WebauthnBuilder};

use crate::id::AccountId;
use crate::key::{KeyId, KeyRole};
use crate::secrets::KeyCredential;
use crate::session::TokenDigest;

/// Adding keys and passkeys and signing in with them, for this instance: every key is bound to the domain it is served at, so moving it voids every key.
#[derive(Debug, Clone)]
pub struct KeyCeremonies {
    webauthn: Webauthn,
    adding: Arc<UnderWay<AccountId, Registration>>,
    answering: Arc<UnderWay<TokenDigest, SecurityKeyAuthentication>>,
    signing_in: Arc<UnderWay<SigningIn, DiscoverableAuthentication>>,
}

/// What the server keeps of a ceremony between asking the browser and hearing back, by the account it is for, the session owing a second step, or a drawn id where nobody is known yet: never written down, so a restart only asks for it to begin again.
#[derive(Debug)]
struct UnderWay<K, S>(Mutex<HashMap<K, (S, OffsetDateTime)>>);

/// A sign-in by passkey under way, before anybody is known: the id it is told by, drawn at random so nobody can answer another's.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SigningIn(String);

impl SigningIn {
    fn drawn() -> Self {
        let mut drawn = [0_u8; 16];
        getrandom::fill(&mut drawn).expect("the system draws random bytes");

        Self(drawn.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn told(told: &str) -> Self {
        Self(told.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Debug for SigningIn {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("SigningIn(…)")
    }
}

/// A key being made, as webauthn-rs asked for it: a passkey or a security key.
#[derive(Debug)]
enum Registration {
    Passkey(PasskeyRegistration),
    SecurityKey(SecurityKeyRegistration),
}

/// Why the browser's answer does not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CeremonyFailed {
    #[error("nothing is under way for this account; begin again")]
    NothingUnderWay,
    #[error("this answer is no key's for this instance")]
    Refused,
}

impl<K, S> Default for UnderWay<K, S> {
    fn default() -> Self {
        Self(Mutex::default())
    }
}

impl<K: Clone + Eq + Hash, S> UnderWay<K, S> {
    /// Holds a ceremony begun, in place of the one before by the same; the oldest gives way when too many are under way.
    fn hold(&self, by: &K, state: S, now: OffsetDateTime) {
        let mut held = self.0.lock().expect("the ceremonies are not poisoned");
        held.retain(|_, (_, begun)| now - *begun < KeyCeremonies::LASTS);
        if held.len() >= KeyCeremonies::MOST_UNDER_WAY
            && let Some(oldest) = held
                .iter()
                .min_by_key(|(_, (_, begun))| *begun)
                .map(|(by, _)| by.clone())
        {
            held.remove(&oldest);
        }
        held.insert(by.clone(), (state, now));
    }

    /// The ceremony, once, if it is still in time.
    fn take(&self, by: &K, now: OffsetDateTime) -> Result<S, CeremonyFailed> {
        self.0
            .lock()
            .expect("the ceremonies are not poisoned")
            .remove(by)
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
                signing_in: Arc::default(),
            })
            .map_err(|_| InvalidPublicUrl::NoUrl)
    }

    /// Asks the browser to make a key in its role, none of the keys the account holds already; a begun adding replaces the one before. A passkey signs in alone, so it must check its user and be kept on the device, found without an address typed. A key for the second step is asked for without credProtect, which browsers refuse for a key that is not resident and only prefers checking its user, and which a second step after the password does not need.
    pub(crate) fn begin_adding(
        &self,
        account: &AccountId,
        role: KeyRole,
        user_name: &str,
        display_name: &str,
        held: &[KeyCredential],
        now: OffsetDateTime,
    ) -> Value {
        let exclude: Vec<CredentialID> = held
            .iter()
            .map(|credential| CredentialID::from(credential.id.clone()))
            .collect();
        let (asked, state) = match role {
            KeyRole::Primary => {
                let (asked, state) = self
                    .webauthn
                    .start_passkey_registration(
                        account.as_uuid(),
                        user_name,
                        display_name,
                        Some(exclude),
                    )
                    .expect("a passkey registration begins for any account");
                let mut asked =
                    serde_json::to_value(asked).expect("what a browser is asked is plain data");
                asked["publicKey"]["authenticatorSelection"]["residentKey"] = "required".into();
                asked["publicKey"]["authenticatorSelection"]["requireResidentKey"] = true.into();

                (asked, Registration::Passkey(state))
            }
            KeyRole::SecondFactor => {
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

                (
                    serde_json::to_value(asked).expect("what a browser is asked is plain data"),
                    Registration::SecurityKey(state),
                )
            }
        };
        self.adding.hold(account, state, now);

        asked
    }

    /// Finishes the account's adding with the browser's answer, once: the credential to keep, or why there is none.
    pub(crate) fn finish_adding(
        &self,
        account: &AccountId,
        answer: &Value,
        now: OffsetDateTime,
    ) -> Result<(KeyRole, KeyCredential), CeremonyFailed> {
        let state = self.adding.take(account, now)?;
        let answer: RegisterPublicKeyCredential =
            serde_json::from_value(answer.clone()).map_err(|_| CeremonyFailed::Refused)?;
        let (role, credential) = match state {
            Registration::Passkey(state) => (
                KeyRole::Primary,
                self.webauthn
                    .finish_passkey_registration(&answer, &state)
                    .map(Credential::from),
            ),
            Registration::SecurityKey(state) => (
                KeyRole::SecondFactor,
                self.webauthn
                    .finish_securitykey_registration(&answer, &state)
                    .map(Credential::from),
            ),
        };
        let credential = credential.map_err(|_| CeremonyFailed::Refused)?;

        Ok((
            role,
            KeyCredential {
                id: credential.cred_id.to_vec(),
                written: serde_json::to_string(&credential).expect("a credential is plain data"),
            },
        ))
    }

    /// Asks the browser for a touch of one of the keys held, to answer the second step this session owes: held by the session, so another signed in by the password asks nothing in its place.
    pub(crate) fn begin_answering(
        &self,
        owing: &TokenDigest,
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
        self.answering.hold(owing, state, now);

        Ok(serde_json::to_value(asked).expect("what a browser is asked is plain data"))
    }

    /// Finishes the session's answering with the browser's answer, once: the key that answered, and its credential as it is now, its counter moved on.
    pub(crate) fn finish_answering(
        &self,
        owing: &TokenDigest,
        answer: &Value,
        held: &[(KeyId, KeyCredential)],
        now: OffsetDateTime,
    ) -> Result<(KeyId, String), CeremonyFailed> {
        let state = self.answering.take(owing, now)?;
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

impl KeyCeremonies {
    /// Asks the browser for any passkey it holds for this instance, nobody named: the id the sign-in goes by, and what the browser is asked. The options do not ask for the browser's autofill, which webauthn-rs sets, so a button can ask too.
    pub(crate) fn begin_signing_in(&self, now: OffsetDateTime) -> (SigningIn, Value) {
        let (asked, state) = self
            .webauthn
            .start_discoverable_authentication()
            .expect("a sign-in by passkey begins for anybody");
        let mut asked = serde_json::to_value(asked).expect("what a browser is asked is plain data");
        if let Some(asked) = asked.as_object_mut() {
            asked.remove("mediation");
        }
        let signing_in = SigningIn::drawn();
        self.signing_in.hold(&signing_in, state, now);

        (signing_in, asked)
    }

    /// Whose passkey the browser's answer names, and which credential of theirs, before anything is checked.
    pub(crate) fn named_by(&self, answer: &Value) -> Result<(AccountId, Vec<u8>), CeremonyFailed> {
        let answer: PublicKeyCredential =
            serde_json::from_value(answer.clone()).map_err(|_| CeremonyFailed::Refused)?;
        let (user, credential) = self
            .webauthn
            .identify_discoverable_authentication(&answer)
            .map_err(|_| CeremonyFailed::Refused)?;

        Ok((AccountId::from(user), credential.to_vec()))
    }

    /// Finishes the sign-in with the browser's answer and the passkey it names, once: its credential as it is now, its counter moved on.
    pub(crate) fn finish_signing_in(
        &self,
        signing_in: &SigningIn,
        answer: &Value,
        held: &KeyCredential,
        now: OffsetDateTime,
    ) -> Result<String, CeremonyFailed> {
        let state = self.signing_in.take(signing_in, now)?;
        let answer: PublicKeyCredential =
            serde_json::from_value(answer.clone()).map_err(|_| CeremonyFailed::Refused)?;
        let credential = read(held).ok_or(CeremonyFailed::Refused)?;
        let answered = self
            .webauthn
            .finish_discoverable_authentication_v2(&answer, state, &credential)
            .map_err(|_| CeremonyFailed::Refused)?;
        let mut renewed = Passkey::from(credential);
        renewed.update_credential(&answered);

        Ok(serde_json::to_string(&Credential::from(renewed)).expect("a credential is plain data"))
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
        let asked = ceremonies.begin_adding(
            &account,
            KeyRole::SecondFactor,
            "ada@example.com",
            "Ada",
            &[],
            clock::at(1_000),
        );

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

    #[test]
    fn a_passkey_is_asked_for_kept_on_the_device_and_checking_its_user() {
        // given: an instance making keys
        let ceremonies =
            KeyCeremonies::serving_at("https://weaveling.example.com").expect("ceremonies");
        let account = AccountId::generate(clock::at(1_000));

        // when: adding a passkey begins
        let asked = ceremonies.begin_adding(
            &account,
            KeyRole::Primary,
            "ada@example.com",
            "Ada",
            &[],
            clock::at(1_000),
        );

        // then: the browser is asked for a resident credential that checks its user
        let selection = &asked["publicKey"]["authenticatorSelection"];
        assert_eq!(selection["residentKey"], "required", "{selection}");
        assert_eq!(selection["requireResidentKey"], true, "{selection}");
        assert_eq!(selection["userVerification"], "required", "{selection}");
    }
}
