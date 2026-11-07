use thiserror::Error;
use webauthn_rs::prelude::Url;
use webauthn_rs::{Webauthn, WebauthnBuilder};

/// Adding keys and passkeys and signing in with them, for this instance: every key is bound to the domain it is served at, so moving it voids every key.
#[derive(Debug, Clone)]
pub struct KeyCeremonies(#[cfg_attr(not(test), expect(dead_code))] Webauthn);

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
            .map(Self)
            .map_err(|_| InvalidPublicUrl::NoUrl)
    }
}

#[cfg(test)]
mod tests {
    use webauthn_rs::prelude::Uuid;

    use super::*;

    fn bound_and_shown_as(ceremonies: &KeyCeremonies) -> (String, String) {
        let (challenge, _) = ceremonies
            .0
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
}
